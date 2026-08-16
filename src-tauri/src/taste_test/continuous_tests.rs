// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;
use rusqlite::Connection;

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
fn test_topic_persona_likelihood_rust() {
    // "rust" should give high likelihood for persona 0 (Rust Systems)
    let rust_likelihood = topic_persona_likelihood("rust", 0);
    let python_likelihood = topic_persona_likelihood("rust", 1);
    assert!(
        rust_likelihood > python_likelihood,
        "Rust topic should favor Rust persona: {rust_likelihood:.2} vs {python_likelihood:.2}"
    );
}
