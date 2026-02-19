/**
 * Tool exports for STREETS MCP Server
 *
 * Course Content Tools:
 * - get_module: Retrieve a course module with all lessons
 * - get_template: Retrieve a worksheet template
 * - search_course: Full-text search across all modules
 * - get_engine: Get details for a specific revenue engine
 *
 * Analysis Tools:
 * - recommend_engines: Analyze project and recommend revenue engines
 * - assess_readiness: Score against the Sovereign Setup checklist
 *
 * Progress Tools:
 * - get_progress: Track completion state across modules
 * - mark_complete: Mark a lesson as complete
 * - get_next_step: Recommend what to work on next
 */

// Course Content Tools
export { getModuleTool, executeGetModule } from "./get-module.js";
export { getTemplateTool, executeGetTemplate } from "./get-template.js";
export { searchCourseTool, executeSearchCourse } from "./search-course.js";
export { getEngineTool, executeGetEngine } from "./get-engine.js";

// Analysis Tools
export { recommendEnginesTool, executeRecommendEngines } from "./recommend-engines.js";
export { assessReadinessTool, executeAssessReadiness } from "./assess-readiness.js";

// Progress Tools
export { getProgressTool, executeGetProgress } from "./get-progress.js";
export { markCompleteTool, executeMarkComplete } from "./mark-complete.js";
export { getNextStepTool, executeGetNextStep } from "./get-next-step.js";
