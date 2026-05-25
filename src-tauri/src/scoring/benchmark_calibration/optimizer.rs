// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Hill-climbing optimizer for sigmoid calibration parameters.

use std::collections::HashMap;
use tracing::info;

use super::runner::run_benchmark_with_embeddings;

/// Optimize sigmoid calibration parameters via greedy hill-climbing.
///
/// Tries 8 neighbors per iteration (center +/- 0.02, scale +/- 0.5, plus 4 diagonals).
/// Accepts the first neighbor that improves accuracy (greedy first-improvement).
/// Max 10 iterations to keep calibration fast.
///
/// Returns (best_center, best_scale, best_accuracy).
pub(super) fn hill_climb_calibration(
    db: &crate::db::Database,
    item_emb: &HashMap<String, Vec<f32>>,
    topic_emb: &HashMap<String, Vec<f32>>,
    start_center: f32,
    start_scale: f32,
    model: &str,
) -> (f32, f32, f32) {
    let mut best_center = start_center;
    let mut best_scale = start_scale;

    // Evaluate starting point
    crate::embedding_calibration::set_active_params(best_center, best_scale);
    let initial_report = run_benchmark_with_embeddings(db, item_emb, topic_emb, model);
    let mut best_accuracy = initial_report.accuracy;

    info!(
        "Hill climb start: center={:.3} scale={:.1} accuracy={:.1}%",
        best_center,
        best_scale,
        best_accuracy * 100.0
    );

    let center_step = 0.02_f32;
    let scale_step = 0.5_f32;

    for iteration in 0..10 {
        // 8 neighbors: 4 cardinal + 4 diagonal
        let neighbors = [
            (best_center + center_step, best_scale),
            (best_center - center_step, best_scale),
            (best_center, best_scale + scale_step),
            (best_center, best_scale - scale_step),
            (best_center + center_step, best_scale + scale_step),
            (best_center + center_step, best_scale - scale_step),
            (best_center - center_step, best_scale + scale_step),
            (best_center - center_step, best_scale - scale_step),
        ];

        let mut improved = false;
        for (nc, ns) in &neighbors {
            // Clamp to reasonable ranges
            let nc = nc.clamp(0.20, 0.70);
            let ns = ns.clamp(5.0, 30.0);

            crate::embedding_calibration::set_active_params(nc, ns);
            let report = run_benchmark_with_embeddings(db, item_emb, topic_emb, model);

            if report.accuracy > best_accuracy {
                best_center = nc;
                best_scale = ns;
                best_accuracy = report.accuracy;
                improved = true;

                info!(
                    "  iter {}: center={:.3} scale={:.1} accuracy={:.1}% (improved)",
                    iteration,
                    best_center,
                    best_scale,
                    best_accuracy * 100.0
                );

                break; // Greedy first-improvement
            }
        }

        if !improved {
            info!("  iter {}: no improvement found, stopping", iteration);
            break;
        }
    }

    // Restore best params
    crate::embedding_calibration::set_active_params(best_center, best_scale);

    (best_center, best_scale, best_accuracy)
}
