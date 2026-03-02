use super::*;
use rusqlite::{params, Connection};

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
