// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Course content loader for STREETS MCP Server
 *
 * Reads markdown files from disk, parses them into structured
 * module/lesson data, and caches the result in memory.
 */

import { readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import type {
  ModuleId,
  TemplateId,
  ModuleContent,
  TemplateContent,
  Lesson,
  EngineDetail,
  SearchResult,
} from "./types.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// =============================================================================
// Module ID to filename mapping
// =============================================================================

const MODULE_FILES: Record<ModuleId, string> = {
  S: "module-s-sovereign-setup.md",
  T: "module-t-technical-moats.md",
  R: "module-r-revenue-engines.md",
  E1: "module-e1-execution-playbook.md",
  E2: "module-e2-evolving-edge.md",
  T2: "module-t2-tactical-automation.md",
  S2: "module-s2-stacking-streams.md",
};

const MODULE_TITLES: Record<ModuleId, string> = {
  S: "Sovereign Setup",
  T: "Technical Moats",
  R: "Revenue Engines",
  E1: "Execution Playbook",
  E2: "Evolving Edge",
  T2: "Tactical Automation",
  S2: "Stacking Streams",
};

const MODULE_DESCRIPTIONS: Record<ModuleId, string> = {
  S: "Configure your rig as business infrastructure. Local LLM stack, legal foundation, and your Sovereign Stack Document.",
  T: "Build skills and positioning that can't be commoditized. Niche selection, moat categories, and your Moat Map.",
  R: "Eight revenue engines with real code, pricing, and platforms. Pick two and build.",
  E1: "Ship your first revenue engine in 48 hours. Pricing, legal setup, distribution, and launch checklist.",
  E2: "Stay ahead of market shifts. 2026 opportunities, timing frameworks, and your Opportunity Radar.",
  T2: "Automate your income streams with LLM pipelines, scheduled jobs, and agent-based systems.",
  S2: "Stack multiple income streams into a resilient portfolio. The $10K/month milestone and 12-month plan.",
};

const FREE_MODULES: Set<ModuleId> = new Set(["S"]);

const TEMPLATE_FILES: Record<TemplateId, { filename: string; title: string }> = {
  "sovereign-stack": {
    filename: "templates/sovereign-stack-document.md",
    title: "Sovereign Stack Document",
  },
  "moat-map": {
    filename: "templates/moat-map.md",
    title: "Moat Map",
  },
  "stream-stack": {
    filename: "templates/stream-stack.md",
    title: "Stream Stack — 12-Month Income Plan",
  },
};

// Engine names from Module R lessons
const ENGINE_NAMES: Record<number, string> = {
  1: "Digital Products",
  2: "Content Monetization",
  3: "Micro-SaaS",
  4: "Automation-as-a-Service",
  5: "API Products",
  6: "Consulting and Fractional CTO",
  7: "Open Source + Premium",
  8: "Data Products and Intelligence",
};

// =============================================================================
// ContentLoader class
// =============================================================================

export class ContentLoader {
  private contentPath: string;
  private moduleCache: Map<ModuleId, ModuleContent> = new Map();
  private templateCache: Map<TemplateId, TemplateContent> = new Map();

  constructor() {
    // Resolve content path: env var, or default to ../docs/streets/ relative to server root
    if (process.env.STREETS_CONTENT_PATH) {
      this.contentPath = process.env.STREETS_CONTENT_PATH;
    } else {
      // __dirname is dist/ when compiled, so go up two levels to reach the repo root
      this.contentPath = join(__dirname, "..", "..", "docs", "streets");
    }
  }

  /**
   * Get the resolved content path (for diagnostics)
   */
  getContentPath(): string {
    return this.contentPath;
  }

  /**
   * Check if a module ID is valid
   */
  isValidModuleId(id: string): id is ModuleId {
    return id in MODULE_FILES;
  }

  /**
   * Check if a template ID is valid
   */
  isValidTemplateId(id: string): id is TemplateId {
    return id in TEMPLATE_FILES;
  }

  /**
   * Get the list of all valid module IDs
   */
  getModuleIds(): ModuleId[] {
    return Object.keys(MODULE_FILES) as ModuleId[];
  }

  /**
   * Get the total number of lessons in a module
   */
  getLessonCount(moduleId: ModuleId): number {
    const mod = this.getModule(moduleId);
    return mod.lessons.length;
  }

  /**
   * Load and parse a module by ID. Results are cached.
   */
  getModule(moduleId: ModuleId): ModuleContent {
    const cached = this.moduleCache.get(moduleId);
    if (cached) return cached;

    const filename = MODULE_FILES[moduleId];
    const filePath = join(this.contentPath, filename);

    if (!existsSync(filePath)) {
      throw new Error(
        `Module file not found: ${filePath}. Set STREETS_CONTENT_PATH to the directory containing the course markdown files.`
      );
    }

    const raw = readFileSync(filePath, "utf-8");
    const lessons = this.parseLessons(raw);

    const moduleContent: ModuleContent = {
      module_id: moduleId,
      title: MODULE_TITLES[moduleId],
      description: MODULE_DESCRIPTIONS[moduleId],
      lessons,
      is_free: FREE_MODULES.has(moduleId),
    };

    this.moduleCache.set(moduleId, moduleContent);
    return moduleContent;
  }

  /**
   * Load a template by ID. Results are cached.
   */
  getTemplate(templateId: TemplateId): TemplateContent {
    const cached = this.templateCache.get(templateId);
    if (cached) return cached;

    const templateDef = TEMPLATE_FILES[templateId];
    const filePath = join(this.contentPath, templateDef.filename);

    if (!existsSync(filePath)) {
      throw new Error(
        `Template file not found: ${filePath}. Set STREETS_CONTENT_PATH to the directory containing the course markdown files.`
      );
    }

    const raw = readFileSync(filePath, "utf-8");

    const templateContent: TemplateContent = {
      template_id: templateId,
      title: templateDef.title,
      content: raw,
    };

    this.templateCache.set(templateId, templateContent);
    return templateContent;
  }

  /**
   * Get a specific revenue engine from Module R
   */
  getEngine(engineNumber: number): EngineDetail {
    if (engineNumber < 1 || engineNumber > 8) {
      throw new Error(`Invalid engine number: ${engineNumber}. Must be 1-8.`);
    }

    const moduleR = this.getModule("R");
    const lessonIdx = engineNumber - 1;

    if (lessonIdx >= moduleR.lessons.length) {
      throw new Error(`Engine ${engineNumber} lesson not found in Module R.`);
    }

    const lesson = moduleR.lessons[lessonIdx];
    const content = lesson.content;

    // Extract time to first dollar and margin from the lesson content
    const timeMatch = content.match(/\*\*Time to first dollar:\*\*\s*(.+)/);
    const marginMatch = content.match(/\*\*Margin:\*\*\s*(.+)/);

    // Extract description: first non-empty paragraph after the lesson header
    const descriptionMatch = content.match(/\*"([^"]+)"\*/);
    const description = descriptionMatch
      ? descriptionMatch[1]
      : `Revenue Engine ${engineNumber}: ${ENGINE_NAMES[engineNumber]}`;

    return {
      engine_number: engineNumber,
      name: ENGINE_NAMES[engineNumber],
      description,
      time_to_first_dollar: timeMatch ? timeMatch[1].trim() : "See lesson content",
      margin: marginMatch ? marginMatch[1].trim() : "See lesson content",
      content,
    };
  }

  /**
   * Full-text search across all modules
   */
  searchCourse(query: string, limit: number = 10): SearchResult[] {
    const results: SearchResult[] = [];
    const queryLower = query.toLowerCase();
    const queryTerms = queryLower.split(/\s+/).filter((t) => t.length > 2);

    for (const moduleId of this.getModuleIds()) {
      try {
        const mod = this.getModule(moduleId);

        for (const lesson of mod.lessons) {
          const contentLower = lesson.content.toLowerCase();
          const titleLower = lesson.title.toLowerCase();

          // Calculate relevance: count term matches, weight title matches higher
          let score = 0;
          for (const term of queryTerms) {
            // Title match (high value)
            if (titleLower.includes(term)) {
              score += 3;
            }

            // Content matches (count occurrences, diminishing returns)
            const regex = new RegExp(escapeRegex(term), "gi");
            const matches = contentLower.match(regex);
            if (matches) {
              score += Math.min(matches.length, 5);
            }
          }

          if (score > 0) {
            // Extract excerpt around first match
            const excerpt = this.extractExcerpt(lesson.content, queryTerms);

            results.push({
              module_id: moduleId,
              lesson_title: lesson.title,
              excerpt,
              relevance_score: score,
            });
          }
        }
      } catch {
        // Skip modules that fail to load
        continue;
      }
    }

    // Sort by relevance descending, then limit
    results.sort((a, b) => b.relevance_score - a.relevance_score);
    return results.slice(0, limit);
  }

  // ===========================================================================
  // Private helpers
  // ===========================================================================

  /**
   * Parse a markdown file into an array of lessons by splitting on ## Lesson headers
   */
  private parseLessons(markdown: string): Lesson[] {
    const lessons: Lesson[] = [];

    // Split on "## Lesson N:" headers
    const parts = markdown.split(/^## Lesson \d+:\s*/m);

    // First part is the module intro (skip it for lesson extraction)
    for (let i = 1; i < parts.length; i++) {
      const part = parts[i];
      const lines = part.split("\n");

      // First line is the lesson title (the rest of the header line)
      const title = lines[0].trim();

      // Rest is the content (everything after the title line)
      const content = lines.slice(1).join("\n").trim();

      lessons.push({ title, content });
    }

    return lessons;
  }

  /**
   * Extract a ~200 character excerpt around the first query term match
   */
  private extractExcerpt(content: string, queryTerms: string[]): string {
    const contentLower = content.toLowerCase();

    // Find earliest match position
    let earliestPos = content.length;
    for (const term of queryTerms) {
      const pos = contentLower.indexOf(term);
      if (pos !== -1 && pos < earliestPos) {
        earliestPos = pos;
      }
    }

    if (earliestPos === content.length) {
      // No match found, return start of content
      return content.substring(0, 200).trim() + "...";
    }

    // Window around the match
    const start = Math.max(0, earliestPos - 60);
    const end = Math.min(content.length, earliestPos + 140);

    let excerpt = content.substring(start, end).trim();

    // Clean up: remove partial markdown formatting at boundaries
    excerpt = excerpt.replace(/^\S*\s/, ""); // remove partial word at start
    if (start > 0) excerpt = "..." + excerpt;
    if (end < content.length) excerpt = excerpt + "...";

    return excerpt;
  }
}

// =============================================================================
// Helpers
// =============================================================================

function escapeRegex(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
