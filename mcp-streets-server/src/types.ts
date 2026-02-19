/**
 * Type definitions for STREETS MCP Server
 *
 * Covers course content, progress tracking, project analysis,
 * and all tool parameter/result interfaces.
 */

// =============================================================================
// Module & Course Content
// =============================================================================

/** Valid STREETS module identifiers */
export type ModuleId = "S" | "T" | "R" | "E1" | "E2" | "T2" | "S2";

/** Valid template identifiers */
export type TemplateId = "sovereign-stack" | "moat-map" | "stream-stack";

/** A parsed lesson from a module markdown file */
export interface Lesson {
  title: string;
  content: string;
}

/** A fully parsed module */
export interface ModuleContent {
  module_id: ModuleId;
  title: string;
  description: string;
  lessons: Lesson[];
  is_free: boolean;
}

/** A template document */
export interface TemplateContent {
  template_id: TemplateId;
  title: string;
  content: string;
}

// =============================================================================
// Revenue Engines (Module R)
// =============================================================================

/** Revenue engine detail from Module R */
export interface EngineDetail {
  engine_number: number;
  name: string;
  description: string;
  time_to_first_dollar: string;
  margin: string;
  content: string;
}

// =============================================================================
// Search
// =============================================================================

/** A search result across course content */
export interface SearchResult {
  module_id: ModuleId;
  lesson_title: string;
  excerpt: string;
  relevance_score: number;
}

// =============================================================================
// Project Analysis
// =============================================================================

/** Detected technology stack from manifest files */
export interface DetectedStack {
  languages: string[];
  frameworks: string[];
  categories: string[];
  dependencies: string[];
}

/** An engine recommendation based on project analysis */
export interface EngineRecommendation {
  engine_number: number;
  name: string;
  match_score: number;
  rationale: string;
  detected_stack: DetectedStack;
}

// =============================================================================
// Readiness Assessment
// =============================================================================

/** A single checklist item in the readiness assessment */
export interface ReadinessItem {
  name: string;
  met: boolean;
  detail: string;
}

/** A category of readiness checks */
export interface ReadinessCategory {
  name: string;
  score: number;
  items: ReadinessItem[];
}

/** Full readiness assessment result */
export interface ReadinessResult {
  overall_score: number;
  categories: ReadinessCategory[];
}

// =============================================================================
// Progress Tracking
// =============================================================================

/** Progress for a single module */
export interface ModuleProgress {
  module_id: string;
  completed_lessons: number;
  total_lessons: number;
  percentage: number;
}

/** Full progress report */
export interface ProgressReport {
  modules: ModuleProgress[];
  overall_percentage: number;
}

/** Result of marking a lesson complete */
export interface MarkCompleteResult {
  success: boolean;
  module_id: string;
  lesson_idx: number;
  module_progress: ModuleProgress;
}

/** Next step recommendation */
export interface NextStepResult {
  next_module_id: ModuleId;
  next_lesson_idx: number;
  reason: string;
  context: string;
}

// =============================================================================
// Tool Parameters
// =============================================================================

export interface GetModuleParams {
  module_id: string;
}

export interface GetTemplateParams {
  template_id: string;
}

export interface SearchCourseParams {
  query: string;
  limit?: number;
}

export interface GetEngineParams {
  engine_number: number;
}

export interface RecommendEnginesParams {
  project_path?: string;
}

export interface AssessReadinessParams {
  project_path?: string;
}

export interface GetProgressParams {
  // No params required
}

export interface MarkCompleteParams {
  module_id: string;
  lesson_idx: number;
}

export interface GetNextStepParams {
  // No params required
}
