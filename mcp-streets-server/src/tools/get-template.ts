// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * get_template tool
 *
 * Retrieve a STREETS worksheet template by ID.
 */

import type { ContentLoader } from "../content.js";
import type { GetTemplateParams, TemplateContent } from "../types.js";

/**
 * Tool definition for MCP registration
 */
export const getTemplateTool = {
  name: "get_template",
  description: `Retrieve a STREETS worksheet template. These are fillable documents that serve as deliverables for specific modules.

Template IDs:
- "sovereign-stack": Sovereign Stack Document (Module S deliverable)
- "moat-map": Moat Map (Module T deliverable)
- "stream-stack": Stream Stack 12-Month Income Plan (Module S2 deliverable)`,
  inputSchema: {
    type: "object" as const,
    properties: {
      template_id: {
        type: "string",
        description: 'Template identifier. One of: "sovereign-stack", "moat-map", "stream-stack"',
        enum: ["sovereign-stack", "moat-map", "stream-stack"],
      },
    },
    required: ["template_id"],
  },
};

/**
 * Execute the get_template tool
 */
export function executeGetTemplate(
  content: ContentLoader,
  params: GetTemplateParams
): TemplateContent {
  const templateId = params.template_id.toLowerCase();

  if (!content.isValidTemplateId(templateId)) {
    throw new Error(
      `Invalid template_id: "${params.template_id}". Valid IDs: sovereign-stack, moat-map, stream-stack`
    );
  }

  return content.getTemplate(templateId);
}
