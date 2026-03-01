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
}
