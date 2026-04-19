// SPDX-License-Identifier: FSL-1.1-Apache-2.0
export interface CalibrationResult {
  grade: string;
  grade_score: number;
  aggregate_f1: number;
  aggregate_precision: number;
  aggregate_recall: number;
  mean_separation_gap: number;
  corpus_items: number;
  personas_tested: number;
  per_persona: PersonaMetrics[];
  worst_persona: string;
  best_persona: string;
  rig_requirements: RigRequirements;
  recommendations: Recommendation[];
  infrastructure_score: number;
  context_richness_score: number;
  signal_coverage_score: number;
  discrimination_score: number;
  active_signal_axes: string[];
  nearest_persona: string;
}

export interface PersonaMetrics {
  name: string;
  display_name: string;
  f1: number;
  precision: number;
  recall: number;
  separation_gap: number;
  tp: number;
  fp: number;
  tn: number;
  fn: number;
}

export interface RigRequirements {
  ollama_running: boolean;
  ollama_url: string;
  embedding_model: string | null;
  embedding_available: boolean;
  gpu_detected: boolean;
  recommended_model: string;
  estimated_ram_gb: number;
  can_reach_grade_a: boolean;
  grade_a_requirements: string[];
}

export interface Recommendation {
  priority: string;
  title: string;
  description: string;
  action: string | null;
  action_type: string | null;
}

// ============================================================================
// Intelligence Mesh — Calibration Curve Fitter (Phase 5b.2)
// ============================================================================
//
// One entry per (model_identity, task) pair that was considered for a
// fit. The UI can render "last fit" tables showing which models/tasks
// got a curve and which were skipped (and why).

export interface CurveFitSummary {
  model_identity_hash: string;
  provider: string;
  model: string;
  task: string;
  samples_scanned: number;
  samples_labeled: number;
  curve_saved: boolean;
  curve_id: string | null;
  brier_score: number | null;
  ece: number | null;
  skipped_reason: string | null;
}

export interface CurveFitReport {
  total_candidates: number;
  curves_produced: number;
  fits: CurveFitSummary[];
}

// Curve status surfaced to the receipts UI. Returned by
// `get_calibration_curve_status`; null means "no curve on disk".
export interface CurveStatus {
  curve_id: string;
  task: string;
  prompt_version: string;
  brier_score: number;
  ece: number;
  sample_count: number;
  created_at: string; // RFC3339
  age_days: number;
  is_stale: boolean;
}

// Drift descriptor emitted on the `calibration-drift` event when the
// rerank loop loads a curve whose prompt_version no longer matches.
export interface CurveDriftEvent {
  curve_id: string;
  task: string;
  model_identity_hash: string;
  stored_prompt_version: string;
  current_prompt_version: string;
  reason: string;
}

// ============================================================================
// Taste Test Calibration
// ============================================================================

export interface TasteCard {
  id: number;
  slot: number;
  title: string;
  snippet: string;
  sourceHint: string;
  categoryHint: string;
}

export interface PersonaWeight {
  name: string;
  weight: number;
}

export interface TasteProfileSummary {
  dominantPersonaName: string;
  dominantPersonaDescription: string;
  confidence: number;
  itemsShown: number;
  personaWeights: PersonaWeight[];
  topInterests: string[];
}

export type TasteTestStepResult =
  | { type: 'nextCard'; card: TasteCard; progress: number; confidence: number }
  | { type: 'complete'; summary: TasteProfileSummary };
