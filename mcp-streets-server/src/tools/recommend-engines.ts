// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * recommend_engines tool
 *
 * Analyze project files and recommend the best revenue engines
 * based on the detected tech stack.
 */

import type { RecommendEnginesParams, EngineRecommendation } from "../types.js";
import { detectStack, recommendEngines } from "../analyzer.js";

/**
 * Tool definition for MCP registration
 */
export const recommendEnginesTool = {
  name: "recommend_engines",
  description: `Analyze a project directory to detect the tech stack and recommend the best STREETS revenue engines.

Reads manifest files (Cargo.toml, package.json, go.mod, pyproject.toml, requirements.txt) to detect languages, frameworks, and dependency categories. Maps the detected stack to the 8 revenue engines with match scores and rationale.

Returns all 8 engines ranked by match score, with the detected stack and reasoning for each recommendation.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      project_path: {
        type: "string",
        description: "Path to the project directory to analyze. Defaults to the current working directory.",
      },
    },
  },
};

/**
 * Execute the recommend_engines tool
 */
export function executeRecommendEngines(
  params: RecommendEnginesParams
): EngineRecommendation[] {
  const projectPath = params.project_path || process.cwd();

  const stack = detectStack(projectPath);

  if (stack.languages.length === 0) {
    throw new Error(
      `No manifest files found in "${projectPath}". ` +
      "Looking for: Cargo.toml, package.json, go.mod, pyproject.toml, or requirements.txt."
    );
  }

  return recommendEngines(stack);
}
