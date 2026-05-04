// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * search_course tool
 *
 * Full-text search across all STREETS Playbook modules.
 */

import type { ContentLoader } from "../content.js";
import type { SearchCourseParams, SearchResult } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const searchCourseTool = {
  name: "search_course",
  description: `Search across all STREETS Playbook modules for specific topics, techniques, or concepts. Returns matching lessons with excerpts and relevance scores.

Searches module titles and full lesson content. Results are ranked by relevance (title matches weighted higher than content matches).`,
  inputSchema: {
    type: "object" as const,
    properties: {
      query: {
        type: "string",
        description: "Search query. Supports multiple terms (all terms are searched independently).",
      },
      limit: {
        type: "number",
        description: "Maximum results to return. Default: 10, max: 50.",
        default: 10,
      },
    },
    required: ["query"],
  },
};

/**
 * Execute the search_course tool
 */
export function executeSearchCourse(
  content: ContentLoader,
  params: SearchCourseParams
): SearchResult[] {
  const query = params.query.trim();
  if (!query) {
    throw new Error("Search query cannot be empty.");
  }

  const limit = Math.max(1, Math.min(50, params.limit ?? 10));

  return content.searchCourse(query, limit);
}
