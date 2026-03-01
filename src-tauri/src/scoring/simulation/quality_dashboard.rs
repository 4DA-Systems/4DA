//! Quality Dashboard — Aggregate quality reporting across all simulation systems
//!
//! Cross-cutting quality tests that span all 9 personas and the full corpus.
//! Individual system tests (reality, lifecycle, differential, first_run) test
//! specific behaviors; the dashboard tests aggregate quality properties.

#[cfg(test)]
mod tests {
    use super::super::super::{score_item, ScoringContext};
    use super::super::corpus::corpus;
    use super::super::metrics::SimMetrics;
    use super::super::personas::all_personas;
    use super::super::{sim_db, sim_input, sim_no_freshness};
    use super::super::{ExpectedOutcome, PERSONA_NAMES};

    // ========================================================================
    // Dashboard report
    // ========================================================================

    #[derive(Debug)]
    struct DashboardReport {
        total_tests_run: u32,
        personas_tested: u32,
        corpus_items_scored: u32,
        aggregate_f1: f64,
        aggregate_precision: f64,
        aggregate_recall: f64,
        mean_separation_gap: f64,
        worst_persona: String,
        worst_persona_f1: f64,
        best_persona: String,
        best_persona_f1: f64,
    }

    // ========================================================================
    // Inline helper (mirrors reality.rs pattern, not imported)
    // ========================================================================

    fn run_persona_simulation(persona_idx: usize, ctx: &ScoringContext) -> SimMetrics {
        let items = corpus();
        let db = sim_db();
        let opts = sim_no_freshness();
        #[cfg(feature = "calibrated-sim")]
        let calibrated_embeddings = super::super::load_corpus_embeddings();
        let zero_emb = vec![0.0_f32; 384];
        let mut metrics = SimMetrics::new();
        for item in &items {
            let expected = item.expected[persona_idx];
            if matches!(expected, ExpectedOutcome::MildBorderline) {
                continue;
            }
            #[cfg(feature = "calibrated-sim")]
            let emb = calibrated_embeddings
                .get((item.id - 1) as usize)
                .unwrap_or(&zero_emb);
            #[cfg(not(feature = "calibrated-sim"))]
            let emb = &zero_emb;
            let input = sim_input(item.id, item.title, item.content, emb);
            let result = score_item(&input, ctx, &db, &opts, None);
            metrics.record(&result, expected);
        }
        metrics
    }

    // ========================================================================
    // Test 1: Every persona produces at least some TP or TN
    // ========================================================================

    #[test]
    fn dashboard_all_personas_produce_metrics() {
        let personas = all_personas();
        for (pi, persona) in personas.iter().enumerate() {
            let m = run_persona_simulation(pi, persona);
            assert!(
                m.tp > 0 || m.tn > 0,
                "Persona {} is completely blind (0 TP and 0 TN)",
                PERSONA_NAMES[pi]
            );
        }
    }

    // ========================================================================
    // Test 2: Aggregate precision across all personas >= 0.40
    // ========================================================================

    #[test]
    fn dashboard_aggregate_precision_above_floor() {
        let personas = all_personas();
        let mut agg = SimMetrics::new();
        for (pi, persona) in personas.iter().enumerate() {
            agg.merge(&run_persona_simulation(pi, persona));
        }
        assert!(
            agg.precision() >= 0.40,
            "Aggregate precision {:.3} below floor 0.40",
            agg.precision()
        );
    }

    // ========================================================================
    // Test 3: No persona with predictions has zero F1
    // ========================================================================

    #[test]
    fn dashboard_no_persona_has_zero_f1() {
        let personas = all_personas();
        for (pi, persona) in personas.iter().enumerate() {
            let m = run_persona_simulation(pi, persona);
            if m.tp + m.fp > 0 {
                assert!(
                    m.f1() > 0.0,
                    "Persona {} has F1=0.0 despite predictions (TP={}, FP={})",
                    PERSONA_NAMES[pi],
                    m.tp,
                    m.fp
                );
            }
        }
    }

    // ========================================================================
    // Test 4: Relevant content scores higher than noise on average
    // ========================================================================

    #[test]
    fn dashboard_separation_gap_positive() {
        let personas = all_personas();
        for (pi, persona) in personas.iter().enumerate() {
            let m = run_persona_simulation(pi, persona);
            if m.relevant_scores.is_empty() {
                continue;
            }
            assert!(
                m.separation_gap() >= 0.0,
                "Persona {} has negative separation gap {:.3}",
                PERSONA_NAMES[pi],
                m.separation_gap()
            );
        }
    }

    // ========================================================================
    // Test 5: Determinism — same items, same persona, identical scores
    // ========================================================================

    #[test]
    fn dashboard_cross_system_consistency() {
        let items = corpus();
        let canonical: Vec<_> = items.iter().take(5).collect();

        let score_run = || -> Vec<f32> {
            let personas = all_personas();
            let db = sim_db();
            let opts = sim_no_freshness();
            let emb = vec![0.0_f32; 384];
            canonical
                .iter()
                .map(|item| {
                    let input = sim_input(item.id, item.title, item.content, &emb);
                    score_item(&input, &personas[0], &db, &opts, None).top_score
                })
                .collect()
        };

        let scores_a = score_run();
        let scores_b = score_run();

        for (i, (a, b)) in scores_a.iter().zip(scores_b.iter()).enumerate() {
            assert!(
                (a - b).abs() < f32::EPSILON,
                "Item {} scored {:.6} vs {:.6} (determinism violation)",
                canonical[i].id,
                a,
                b
            );
        }
    }

    // ========================================================================
    // Test 6: Full dashboard report
    // ========================================================================

    #[test]
    fn dashboard_full_report() {
        let personas = all_personas();
        let items = corpus();
        let mut aggregate = SimMetrics::new();
        let mut per_persona: Vec<(String, SimMetrics)> = Vec::new();
        let mut total_scored: u32 = 0;

        for (pi, persona) in personas.iter().enumerate() {
            let m = run_persona_simulation(pi, persona);
            total_scored += m.tp + m.fp + m.tn + m.r#fn;
            per_persona.push((PERSONA_NAMES[pi].to_string(), m));
        }

        let mut best = ("none".to_string(), 0.0_f64);
        let mut worst = ("none".to_string(), 1.0_f64);
        let mut gap_sum = 0.0_f64;
        let mut gap_count = 0u32;

        for (name, m) in &per_persona {
            let f = m.f1();
            if f > best.1 {
                best = (name.clone(), f);
            }
            if f < worst.1 {
                worst = (name.clone(), f);
            }
            if !m.relevant_scores.is_empty() {
                gap_sum += m.separation_gap();
                gap_count += 1;
            }
            aggregate.merge(m);
        }

        let report = DashboardReport {
            total_tests_run: 6,
            personas_tested: PERSONA_NAMES.len() as u32,
            corpus_items_scored: total_scored,
            aggregate_f1: aggregate.f1(),
            aggregate_precision: aggregate.precision(),
            aggregate_recall: aggregate.recall(),
            mean_separation_gap: if gap_count > 0 {
                gap_sum / gap_count as f64
            } else {
                0.0
            },
            worst_persona: worst.0,
            worst_persona_f1: worst.1,
            best_persona: best.0,
            best_persona_f1: best.1,
        };

        println!("\n{}", "=".repeat(72));
        println!("  QUALITY DASHBOARD -- Scoring Simulation Report");
        println!("{}", "=".repeat(72));
        println!("  Corpus size:       {} items", items.len());
        println!("  Personas tested:   {}", report.personas_tested);
        println!(
            "  Items scored:      {} (across all personas)",
            report.corpus_items_scored
        );
        println!("{}", "-".repeat(72));
        println!("  AGGREGATE METRICS");
        println!("    Precision:       {:.3}", report.aggregate_precision);
        println!("    Recall:          {:.3}", report.aggregate_recall);
        println!("    F1:              {:.3}", report.aggregate_f1);
        println!("    Mean Sep. Gap:   {:.3}", report.mean_separation_gap);
        println!("{}", "-".repeat(72));
        println!("  PER-PERSONA BREAKDOWN");
        for (name, m) in &per_persona {
            println!(
                "    {:<18} P={:.3}  R={:.3}  F1={:.3}  gap={:.3}",
                name,
                m.precision(),
                m.recall(),
                m.f1(),
                m.separation_gap()
            );
        }
        println!("{}", "-".repeat(72));
        println!(
            "  Best persona:      {} (F1={:.3})",
            report.best_persona, report.best_persona_f1
        );
        println!(
            "  Worst persona:     {} (F1={:.3})",
            report.worst_persona, report.worst_persona_f1
        );
        println!("{}", "=".repeat(72));
    }
}
