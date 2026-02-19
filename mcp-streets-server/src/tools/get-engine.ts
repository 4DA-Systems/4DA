/**
 * get_engine tool
 *
 * Get details for a specific revenue engine from Module R.
 */

import type { ContentLoader } from "../content.js";
import type { GetEngineParams, EngineDetail } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const getEngineTool = {
  name: "get_engine",
  description: `Get detailed information about a specific STREETS revenue engine from Module R.

Engines:
1. Digital Products — highest margin, lowest risk
2. Content Monetization — newsletters, courses, technical writing
3. Micro-SaaS — small, focused software products
4. Automation-as-a-Service — sell automated workflows
5. API Products — wrap capabilities as paid APIs
6. Consulting and Fractional CTO — high-rate expertise
7. Open Source + Premium — open core with paid features
8. Data Products and Intelligence — sell insights and datasets

Returns the engine name, description, time to first dollar, margin, and full lesson content.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      engine_number: {
        type: "number",
        description: "Engine number (1-8). Each number corresponds to a lesson in Module R.",
        minimum: 1,
        maximum: 8,
      },
    },
    required: ["engine_number"],
  },
};

/**
 * Execute the get_engine tool
 */
export function executeGetEngine(
  content: ContentLoader,
  params: GetEngineParams
): EngineDetail {
  const engineNumber = params.engine_number;

  if (!Number.isInteger(engineNumber) || engineNumber < 1 || engineNumber > 8) {
    throw new Error(
      `Invalid engine_number: ${engineNumber}. Must be an integer from 1 to 8.`
    );
  }

  return content.getEngine(engineNumber);
}
