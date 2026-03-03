//! System 1: Multi-Session Convergence
//!
//! Validates that scoring quality improves (or at least doesn't degrade)
//! over 20 simulated feedback sessions.

use std::collections::HashMap;

use super::super::score_item;
use super::feedback_sim::{
    apply_feedback, lifecycle_corpus, python_ctx_with_boosts, rust_ctx_with_boosts,
    simulate_session_with_embeddings,
};
use super::metrics::SimMetrics;
use super::{sim_db, sim_input, sim_no_freshness};
use super::{ExpectedOutcome, PERSONA_NAMES};

const N_SESSIONS: usize = 20;

// ============================================================================
// Session runners
// ============================================================================

fn run_rust_lifecycle_sessions(n: usize) -> Vec<f64> {
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
    let mut boosts: HashMap<String, f64> = HashMap::new();
    let mut f1_per_session = Vec::new();

    for session_idx in 0..n {
        let interaction_count = (session_idx as i64 + 1) * 10;
        let ctx = rust_ctx_with_boosts(&boosts, interaction_count);

        // Measure quality this session
        let db = sim_db();
        let opts = sim_no_freshness();
        let mut metrics = SimMetrics::new();

        for item in &items {
            let expected = item.expected[0]; // rust persona
            if matches!(expected, ExpectedOutcome::MildBorderline) {
                continue;
            }
            let emb = calibrated_embeddings
                .get((item.id - 1) as usize)
                .unwrap_or(&zero_emb);
            let input = sim_input(item.id, item.title, item.content, emb);
            let result = score_item(&input, &ctx, &db, &opts, None);
            metrics.record(&result, expected);
        }
        f1_per_session.push(metrics.f1());

        // Generate feedback for next session
        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }
    f1_per_session
}

fn run_python_lifecycle_sessions(n: usize) -> Vec<f64> {
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
    let mut boosts: HashMap<String, f64> = HashMap::new();
    let mut f1_per_session = Vec::new();

    for session_idx in 0..n {
        let interaction_count = (session_idx as i64 + 1) * 10;
        let ctx = python_ctx_with_boosts(&boosts, interaction_count);

        let db = sim_db();
        let opts = sim_no_freshness();
        let mut metrics = SimMetrics::new();

        for item in &items {
            let expected = item.expected[1]; // python persona
            if matches!(expected, ExpectedOutcome::MildBorderline) {
                continue;
            }
            let emb = calibrated_embeddings
                .get((item.id - 1) as usize)
                .unwrap_or(&zero_emb);
            let input = sim_input(item.id, item.title, item.content, emb);
            let result = score_item(&input, &ctx, &db, &opts, None);
            metrics.record(&result, expected);
        }
        f1_per_session.push(metrics.f1());

        let events = simulate_session_with_embeddings(&ctx, &items, 1, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }
    f1_per_session
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn lifecycle_rust_persona_does_not_degrade() {
    let f1s = run_rust_lifecycle_sessions(N_SESSIONS);
    assert!(!f1s.is_empty(), "No sessions ran");

    let first = f1s[0];
    let last = f1s[f1s.len() - 1];

    println!("[lifecycle_rust] first_f1={first:.3} last_f1={last:.3}");

    // Last session F1 must be at least 85% of first session F1
    let min_acceptable = first * 0.85;
    assert!(last >= min_acceptable,
        "Rust persona degraded over {N_SESSIONS} sessions: first={first:.3} last={last:.3} min_acceptable={min_acceptable:.3}");
}

#[test]
fn lifecycle_rust_convergence_trend() {
    let f1s = run_rust_lifecycle_sessions(N_SESSIONS);
    assert!(
        f1s.len() >= 4,
        "Need at least 4 sessions for trend analysis"
    );

    let half = f1s.len() / 2;
    let first_half_avg: f64 = f1s[..half].iter().sum::<f64>() / half as f64;
    let second_half_avg: f64 = f1s[half..].iter().sum::<f64>() / (f1s.len() - half) as f64;

    println!(
        "[lifecycle_rust_trend] first_half={first_half_avg:.3} second_half={second_half_avg:.3}"
    );

    // Second half should be >= 90% of first half (no catastrophic degradation)
    assert!(second_half_avg >= first_half_avg * 0.90,
        "Rust persona shows degradation trend: first_half={first_half_avg:.3} second_half={second_half_avg:.3}");
}

#[test]
fn lifecycle_python_persona_does_not_degrade() {
    let f1s = run_python_lifecycle_sessions(N_SESSIONS);
    assert!(!f1s.is_empty(), "No sessions ran");

    let first = f1s[0];
    let last = f1s[f1s.len() - 1];

    println!("[lifecycle_python] first_f1={first:.3} last_f1={last:.3}");

    let min_acceptable = first * 0.85;
    assert!(last >= min_acceptable,
        "Python persona degraded over {N_SESSIONS} sessions: first={first:.3} last={last:.3} min={min_acceptable:.3}");
}

#[test]
fn lifecycle_cross_persona_isolation_holds() {
    // Run Rust lifecycle and then verify Python content stays irrelevant
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
    let mut rust_boosts: HashMap<String, f64> = HashMap::new();

    // Run 10 sessions of Rust feedback
    for session_idx in 0..10 {
        let interaction_count = (session_idx as i64 + 1) * 10;
        let ctx = rust_ctx_with_boosts(&rust_boosts, interaction_count);
        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        rust_boosts = apply_feedback(&rust_boosts, &events);
    }

    // After 10 Rust sessions, Python content should still be scored low
    let final_rust_ctx = rust_ctx_with_boosts(&rust_boosts, 100);
    let db = sim_db();
    let opts = sim_no_freshness();

    let mut python_false_positives = 0u32;
    let mut python_total = 0u32;

    for item in &items {
        if item.expected[0] != ExpectedOutcome::NotRelevant {
            continue; // only look at items that should be noise for Rust
        }
        if item.expected[1] != ExpectedOutcome::StrongRelevant {
            continue; // only look at items that are strong for Python
        }
        let emb = calibrated_embeddings
            .get((item.id - 1) as usize)
            .unwrap_or(&zero_emb);
        let input = sim_input(item.id, item.title, item.content, emb);
        let result = score_item(&input, &final_rust_ctx, &db, &opts, None);
        python_total += 1;
        if result.relevant {
            python_false_positives += 1;
        }
    }

    if python_total > 0 {
        let fp_rate = python_false_positives as f64 / python_total as f64;
        assert!(fp_rate <= 0.40,
            "Cross-persona isolation failed after Rust lifecycle: {python_false_positives}/{python_total} Python items scored as relevant ({fp_rate:.2})");
    }
}

#[test]
fn lifecycle_feedback_boosts_stay_bounded() {
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let mut boosts: HashMap<String, f64> = HashMap::new();

    for session_idx in 0..N_SESSIONS {
        let ctx = rust_ctx_with_boosts(&boosts, (session_idx as i64 + 1) * 10);
        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }

    // After all sessions, no boost should exceed [-1.0, 1.0]
    for (topic, &boost) in &boosts {
        assert!(
            boost >= -1.0 && boost <= 1.0,
            "Boost for '{topic}' out of bounds: {boost:.3}"
        );
    }
}

#[test]
fn lifecycle_feedback_boosts_do_not_saturate() {
    // After 20 sessions, boosts should not all be at ±1.0 (clamped)
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let mut boosts: HashMap<String, f64> = HashMap::new();

    for session_idx in 0..N_SESSIONS {
        let ctx = rust_ctx_with_boosts(&boosts, (session_idx as i64 + 1) * 10);
        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }

    if boosts.is_empty() {
        return; // No boosts generated — test is vacuously true
    }

    let saturated = boosts.values().filter(|&&v| v.abs() >= 0.99).count();
    let total = boosts.len();
    let saturation_rate = saturated as f64 / total as f64;

    // With few topics, all boosts may naturally reach high values.
    // Only flag saturation when there are enough boosts to judge.
    if total < 3 {
        return; // Too few boosts to meaningfully assess saturation
    }
    assert!(saturation_rate < 0.8,
        "Too many boosts saturated ({saturated}/{total} = {saturation_rate:.2}). The decay factor may be too low.");
}

#[test]
fn lifecycle_same_item_stable_score_over_sessions() {
    // A fixed item scored against the same-session context should be stable
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let probe_input = sim_input(
        999,
        "Rust memory safety and ownership",
        "Rust's ownership model prevents use-after-free and data races at compile time.",
        &emb,
    );

    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let mut boosts: HashMap<String, f64> = HashMap::new();
    let mut probe_scores = Vec::new();

    for session_idx in 0..5 {
        let ctx = rust_ctx_with_boosts(&boosts, (session_idx as i64 + 1) * 10);
        let result = score_item(&probe_input, &ctx, &db, &opts, None);
        probe_scores.push(result.top_score);

        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }

    // Score should not oscillate wildly
    let min = probe_scores.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = probe_scores
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    let range = (max - min) as f64;

    println!("[lifecycle_stable] probe scores: {probe_scores:?} range={range:.3}");
    assert!(range <= 0.4,
        "Probe item score oscillated too much over sessions: min={min:.3} max={max:.3} range={range:.3}");
}

#[test]
fn lifecycle_noise_stays_rejected_across_sessions() {
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
    let db = sim_db();
    let opts = sim_no_freshness();
    let mut boosts: HashMap<String, f64> = HashMap::new();

    // Run all sessions
    for session_idx in 0..N_SESSIONS {
        let ctx = rust_ctx_with_boosts(&boosts, (session_idx as i64 + 1) * 10);
        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }

    // After final session, noise should still be mostly rejected
    let final_ctx = rust_ctx_with_boosts(&boosts, (N_SESSIONS as i64) * 10);
    let mut noise_relevant = 0u32;
    let mut noise_total = 0u32;

    for item in &items {
        if item.expected[0] != ExpectedOutcome::NotRelevant {
            continue;
        }
        let emb = calibrated_embeddings
            .get((item.id - 1) as usize)
            .unwrap_or(&zero_emb);
        let input = sim_input(item.id, item.title, item.content, emb);
        let result = score_item(&input, &final_ctx, &db, &opts, None);
        noise_total += 1;
        if result.relevant {
            noise_relevant += 1;
        }
    }

    if noise_total > 0 {
        let noise_fp_rate = noise_relevant as f64 / noise_total as f64;
        assert!(noise_fp_rate <= 0.35,
            "Noise rejection degraded after {N_SESSIONS} sessions: {noise_relevant}/{noise_total} noise items relevant ({noise_fp_rate:.2})");
    }
}

#[test]
fn lifecycle_aggregate_report() {
    println!("\n=== LIFECYCLE AGGREGATE REPORT ===");
    let rust_f1s = run_rust_lifecycle_sessions(N_SESSIONS);
    let python_f1s = run_python_lifecycle_sessions(N_SESSIONS);

    println!("Rust F1 trajectory (sessions 1-{N_SESSIONS}):");
    for (i, f1) in rust_f1s.iter().enumerate() {
        print!("  session {}: {f1:.3}", i + 1);
        if (i + 1) % 5 == 0 {
            println!();
        }
    }
    println!();

    println!("Python F1 trajectory (sessions 1-{N_SESSIONS}):");
    for (i, f1) in python_f1s.iter().enumerate() {
        print!("  session {}: {f1:.3}", i + 1);
        if (i + 1) % 5 == 0 {
            println!();
        }
    }
    println!();

    let rust_final = rust_f1s.last().copied().unwrap_or(0.0);
    let python_final = python_f1s.last().copied().unwrap_or(0.0);

    println!("Final F1: rust={rust_final:.3} python={python_final:.3}");
    println!("Personas tested: {}", PERSONA_NAMES.join(", "));
}

#[test]
fn persona_builders_produce_valid_contexts() {
    use super::personas::all_personas;
    let personas = all_personas();
    assert_eq!(personas.len(), 9, "Expected 9 personas");

    for (i, persona) in personas.iter().enumerate() {
        assert!(
            persona.interest_count <= 10,
            "Persona {i} has too many interests: {}",
            persona.interest_count
        );
        assert!(
            persona.feedback_interaction_count >= 0,
            "Persona {i} has negative interaction count"
        );
    }
}
