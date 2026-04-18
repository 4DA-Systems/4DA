// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * assess_readiness tool
 *
 * Score a project against the Sovereign Setup checklist from Module S.
 */

import type { AssessReadinessParams, ReadinessResult } from "../types.js";
import { assessReadiness } from "../analyzer.js";

/**
 * Tool definition for MCP registration
 */
export const assessReadinessTool = {
  name: "assess_readiness",
  description: `Assess a project's readiness against the STREETS Sovereign Setup checklist (Module S).

Checks four categories:
- local_llm: Ollama installation, LLM configuration
- legal_foundation: License, terms of service, privacy policy
- development_env: Git, CI/CD, README, code formatting
- revenue_infrastructure: Payment SDK, landing page, analytics

Returns an overall score (0-100) and per-category scores with detailed item-level results.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      project_path: {
        type: "string",
        description: "Path to the project directory to assess. Defaults to the current working directory.",
      },
    },
  },
};

/**
 * Execute the assess_readiness tool
 */
export function executeAssessReadiness(
  params: AssessReadinessParams
): ReadinessResult {
  const projectPath = params.project_path || process.cwd();
  return assessReadiness(projectPath);
}
