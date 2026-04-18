// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Score Autopsy Tool
 *
 * Deep forensic analysis of why content scored the way it did.
 * This is a SUPERPOWER - it reveals the complete scoring breakdown.
 *
 * With synthesize=true, uses LLM to explain whether the score makes sense.
 */

import type { FourDADatabase } from "../db.js";
import { getLLMConfig, canSynthesize, synthesize, SYNTHESIS_PROMPTS } from "../llm.js";
import { createScoreAutopsyCompact, type CompactResult, type ScoreAutopsyKeyData } from "../output-manager.js";

export const scoreAutopsyTool = {
  name: "score_autopsy",
  description: `Perform a complete forensic analysis of why an item scored the way it did.

With synthesize=true (recommended), uses AI to explain whether the score seems correct.

Returns:
- Component-by-component score breakdown
- Matching context (interests, tech, topics)
- Comparison with similar items
- Recommendations for improving relevance
- AI analysis (when enabled)

Use this when you need to understand or debug relevance scores.`,
  inputSchema: {
    type: "object",
    properties: {
      item_id: {
        type: "number",
        description: "The item ID to analyze",
      },
      source_type: {
        type: "string",
        description: "Source type (e.g., 'hackernews', 'arxiv')",
      },
      compare_count: {
        type: "number",
        description: "Number of similar items to compare (default: 5)",
      },
      synthesize: {
        type: "boolean",
        description: "Use AI to analyze if the score makes sense (default: true if LLM configured)",
      },
      compact: {
        type: "boolean",
        description: "Return compact result with file reference (default: true for ~80% token reduction)",
      },
    },
    required: ["item_id", "source_type"],
  },
};

export interface ScoreAutopsyParams {
  item_id: number;
  source_type: string;
  compare_count?: number;
  synthesize?: boolean;
  compact?: boolean;
}

interface ScoreComponent {
  name: string;
  raw_value: number;
  weight: number;
  contribution: number;
  explanation: string;
}

interface SimilarItem {
  id: number;
  title: string;
  score: number;
  score_difference: number;
  key_difference: string;
}

interface AutopsyResult {
  item: {
    id: number;
    title: string;
    url: string | null;
    source_type: string;
    created_at: string;
    age_hours: number;
  };
  final_score: number;
  components: ScoreComponent[];
  matching_context: {
    interests: string[];
    tech_stack: string[];
    active_topics: string[];
    learned_affinities: string[];
    exclusions_hit: string[];
  };
  similar_items: SimilarItem[];
  recommendations: string[];
  narrative: string;
  // AI-powered analysis
  ai_analysis?: {
    verdict: string;
    score_assessment: "accurate" | "too_high" | "too_low" | "uncertain";
    reasoning: string;
    suggested_action: string;
    model_used: string;
  };
}

export async function executeScoreAutopsy(
  db: FourDADatabase,
  params: ScoreAutopsyParams
): Promise<AutopsyResult | CompactResult<ScoreAutopsyKeyData>> {
  const { item_id, source_type, compare_count = 5 } = params;
  const useCompact = params.compact !== false; // Default to compact=true

  // Check LLM availability
  const dbInstance = (db as unknown as { db: { prepare: (sql: string) => { get: (...args: unknown[]) => unknown } } }).db;
  const llmConfig = getLLMConfig(dbInstance);
  const shouldSynthesize = params.synthesize ?? canSynthesize(llmConfig);

  // Get the item
  const item = db.getSourceItem(item_id, source_type);
  if (!item) {
    throw new Error(`Item ${item_id} of type ${source_type} not found`);
  }

  // Get full context
  const context = db.getUserContext(true, true);

  // Get detailed explanation
  const explanation = db.explainRelevance(item_id, source_type);
  if (!explanation) {
    throw new Error("Failed to compute relevance explanation");
  }

  // Calculate age
  const createdAt = new Date(item.created_at.replace(" ", "T") + "Z");
  const ageHours = Math.round((Date.now() - createdAt.getTime()) / (1000 * 60 * 60));

  // Build component breakdown
  const components: ScoreComponent[] = [
    {
      name: "Interest Match",
      raw_value: explanation.matching_context.matching_interests.length > 0 ? 0.8 : 0,
      weight: 0.30,
      contribution: explanation.score_breakdown.static_match_score * 0.5,
      explanation: explanation.matching_context.matching_interests.length > 0
        ? `Matches interests: ${explanation.matching_context.matching_interests.join(", ")}`
        : "No direct interest matches",
    },
    {
      name: "Tech Stack Match",
      raw_value: explanation.matching_context.matching_tech.length > 0 ? 0.7 : 0,
      weight: 0.20,
      contribution: explanation.matching_context.matching_tech.length * 0.2,
      explanation: explanation.matching_context.matching_tech.length > 0
        ? `Matches tech: ${explanation.matching_context.matching_tech.join(", ")}`
        : "No tech stack matches",
    },
    {
      name: "Active Topics (ACE)",
      raw_value: explanation.score_breakdown.ace_match_score,
      weight: 0.15,
      contribution: explanation.score_breakdown.ace_match_score,
      explanation: explanation.matching_context.matching_topics.length > 0
        ? `Recent work topics: ${explanation.matching_context.matching_topics.join(", ")}`
        : "No active topic matches",
    },
    {
      name: "Learned Affinities",
      raw_value: explanation.score_breakdown.learned_affinity_score,
      weight: 0.15,
      contribution: explanation.score_breakdown.learned_affinity_score,
      explanation: explanation.matching_context.matching_affinities.length > 0
        ? `Learned preferences: ${explanation.matching_context.matching_affinities.join(", ")}`
        : "No learned affinity matches",
    },
    {
      name: "Recency Boost",
      raw_value: ageHours < 24 ? 0.9 : ageHours < 72 ? 0.6 : 0.3,
      weight: 0.10,
      contribution: ageHours < 24 ? 0.09 : ageHours < 72 ? 0.06 : 0.03,
      explanation: ageHours < 24
        ? `Fresh content (${ageHours}h old)`
        : `Older content (${Math.round(ageHours / 24)}d old)`,
    },
    {
      name: "Anti-Penalty",
      raw_value: -explanation.score_breakdown.anti_penalty,
      weight: 1.0,
      contribution: -explanation.score_breakdown.anti_penalty,
      explanation: explanation.score_breakdown.anti_penalty > 0
        ? "Score reduced due to exclusions or anti-topics"
        : "No penalties applied",
    },
  ];

  // Get similar items for comparison
  const similarItems = getSimilarItems(db, item, source_type, compare_count);

  // Generate recommendations
  const recommendations = generateRecommendations(
    explanation.score_breakdown,
    explanation.matching_context,
    context,
    item
  );

  // Generate narrative explanation
  const narrative = generateNarrative(
    item,
    explanation.score_breakdown.final_score,
    explanation.matching_context,
    ageHours
  );

  const result: AutopsyResult = {
    item: {
      id: item.id,
      title: item.title,
      url: item.url,
      source_type: item.source_type,
      created_at: item.created_at,
      age_hours: ageHours,
    },
    final_score: explanation.score_breakdown.final_score,
    components,
    matching_context: {
      interests: explanation.matching_context.matching_interests,
      tech_stack: explanation.matching_context.matching_tech,
      active_topics: explanation.matching_context.matching_topics,
      learned_affinities: explanation.matching_context.matching_affinities,
      exclusions_hit: explanation.score_breakdown.anti_penalty > 0
        ? ["Exclusion or anti-topic triggered"]
        : [],
    },
    similar_items: similarItems,
    recommendations,
    narrative,
  };

  // AI Analysis - the actual superpower
  if (shouldSynthesize && canSynthesize(llmConfig)) {
    try {
      const autopsyData = {
        item: { title: item.title, content: item.content?.substring(0, 500) },
        final_score: result.final_score,
        components: result.components,
        matching_context: result.matching_context,
      };

      const contextData = {
        interests: context.interests.slice(0, 10),
        tech_stack: context.tech_stack,
        role: context.role,
      };

      const synthesis = await synthesize(llmConfig, {
        system: SYNTHESIS_PROMPTS.scoreAutopsy.system,
        prompt: SYNTHESIS_PROMPTS.scoreAutopsy.buildPrompt(autopsyData, contextData),
        max_tokens: 350,
        complexity: SYNTHESIS_PROMPTS.scoreAutopsy.complexity,
      });

      // Parse the synthesis to extract structured assessment
      const text = synthesis.synthesis.toLowerCase();
      let scoreAssessment: "accurate" | "too_high" | "too_low" | "uncertain" = "uncertain";
      if (text.includes("too high") || text.includes("overscored") || text.includes("inflated")) {
        scoreAssessment = "too_high";
      } else if (text.includes("too low") || text.includes("underscored") || text.includes("should be higher")) {
        scoreAssessment = "too_low";
      } else if (text.includes("makes sense") || text.includes("accurate") || text.includes("correct") || text.includes("appropriate")) {
        scoreAssessment = "accurate";
      }

      result.ai_analysis = {
        verdict: synthesis.synthesis,
        score_assessment: scoreAssessment,
        reasoning: synthesis.synthesis.split("\n")[0] || "",
        suggested_action: recommendations[0] || "No specific action needed",
        model_used: synthesis.model_used,
      };
    } catch (error) {
      console.error("AI analysis failed:", error);
    }
  }

  // Return compact or full result based on parameter
  if (useCompact) {
    return createScoreAutopsyCompact(result);
  }

  return result;
}

function getSimilarItems(
  db: FourDADatabase,
  item: { id: number; title: string; source_type: string },
  sourceType: string,
  count: number
): SimilarItem[] {
  // Get recent items from same source to compare
  const recentItems = db.getRelevantContent(0, sourceType, count * 2, 168); // Last week

  return recentItems
    .filter((ri) => ri.id !== item.id)
    .slice(0, count)
    .map((ri) => {
      const scoreDiff = ri.relevance_score - db.explainRelevance(item.id, sourceType)!.score_breakdown.final_score;
      return {
        id: ri.id,
        title: ri.title,
        score: ri.relevance_score,
        score_difference: Math.round(scoreDiff * 100) / 100,
        key_difference: scoreDiff > 0
          ? "Higher: likely more keyword matches"
          : "Lower: fewer context matches",
      };
    });
}

function generateRecommendations(
  scores: { static_match_score: number; ace_match_score: number; learned_affinity_score: number },
  matching: { matching_interests: string[]; matching_tech: string[]; matching_topics: string[]; matching_affinities: string[] },
  context: { interests: { topic: string }[]; tech_stack: string[] },
  item: { title: string; content: string }
): string[] {
  const recommendations: string[] = [];

  // Low interest match
  if (matching.matching_interests.length === 0 && scores.static_match_score < 0.2) {
    // Extract potential keywords from item
    const words = item.title.toLowerCase().split(/\s+/).filter(w => w.length > 4);
    const topWord = words[0] || "this topic";
    recommendations.push(
      `Add "${topWord}" as an explicit interest to boost similar content`
    );
  }

  // Low tech match
  if (matching.matching_tech.length === 0 && context.tech_stack.length > 0) {
    recommendations.push(
      "Item doesn't mention your tech stack - relevance may improve if content aligns with: " +
      context.tech_stack.slice(0, 3).join(", ")
    );
  }

  // Low ACE match
  if (scores.ace_match_score < 0.05) {
    recommendations.push(
      "No recent work context matches - ensure ACE is scanning your active projects"
    );
  }

  // Low learned score
  if (scores.learned_affinity_score < 0.05) {
    recommendations.push(
      "Give feedback on similar items to help 4DA learn your preferences"
    );
  }

  // General recommendation if score is decent
  if (recommendations.length === 0) {
    recommendations.push("Score looks healthy - continue providing feedback to refine");
  }

  return recommendations;
}

function generateNarrative(
  item: { title: string },
  score: number,
  matching: { matching_interests: string[]; matching_tech: string[]; matching_topics: string[] },
  ageHours: number
): string {
  const parts: string[] = [];

  // Score assessment
  if (score >= 0.8) {
    parts.push(`"${item.title}" is highly relevant to you (${Math.round(score * 100)}% match).`);
  } else if (score >= 0.5) {
    parts.push(`"${item.title}" has moderate relevance (${Math.round(score * 100)}% match).`);
  } else {
    parts.push(`"${item.title}" has low relevance (${Math.round(score * 100)}% match).`);
  }

  // Why
  const matches = [
    ...matching.matching_interests.map(i => `interest "${i}"`),
    ...matching.matching_tech.map(t => `tech "${t}"`),
    ...matching.matching_topics.map(t => `recent work on "${t}"`),
  ];

  if (matches.length > 0) {
    parts.push(`It matches your ${matches.slice(0, 2).join(" and ")}.`);
  } else {
    parts.push("No strong matches to your declared interests or recent work.");
  }

  // Recency
  if (ageHours < 24) {
    parts.push("The freshness boost helps (published within 24 hours).");
  }

  return parts.join(" ");
}
