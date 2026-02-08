/**
 * Daily Briefing Tool
 *
 * Generate an executive summary of 4DA's discoveries.
 * This is a SUPERPOWER - it synthesizes noise into actionable intelligence.
 *
 * With synthesize=true, uses LLM to generate genuine insights.
 */

import type { FourDADatabase } from "../db.js";
import { getLLMConfig, canSynthesize, synthesize, SYNTHESIS_PROMPTS } from "../llm.js";
import { createDailyBriefingCompact, type CompactResult, type DailyBriefingKeyData } from "../output-manager.js";

export const dailyBriefingTool = {
  name: "daily_briefing",
  description: `Generate an executive briefing from 4DA's recent discoveries.

With synthesize=true (recommended), uses AI to generate genuine insights tailored to your context.

Returns:
- TL;DR summary (AI-generated when synthesize=true)
- Key themes with item counts
- Notable high-scoring items
- Actionable recommendations
- Trend highlights
- AI synthesis (when enabled)

Use this for your daily/weekly content review.`,
  inputSchema: {
    type: "object",
    properties: {
      hours: {
        type: "number",
        description: "Hours to look back (default: 24)",
      },
      min_score: {
        type: "number",
        description: "Minimum relevance score (default: 0.5)",
      },
      max_items: {
        type: "number",
        description: "Maximum items to analyze (default: 50)",
      },
      format: {
        type: "string",
        enum: ["full", "brief", "actionable"],
        description: "Output format (default: full)",
      },
      synthesize: {
        type: "boolean",
        description: "Use AI to generate genuine insights (default: true if LLM configured)",
      },
      compact: {
        type: "boolean",
        description: "Return compact result with file reference (default: true for ~80% token reduction)",
      },
    },
  },
};

export interface DailyBriefingParams {
  hours?: number;
  min_score?: number;
  max_items?: number;
  format?: "full" | "brief" | "actionable";
  synthesize?: boolean;
  compact?: boolean;
}

interface Theme {
  name: string;
  item_count: number;
  avg_score: number;
  top_items: { title: string; score: number; url: string | null }[];
  insight: string;
}

interface BriefingResult {
  period: {
    start: string;
    end: string;
    hours: number;
  };
  summary: {
    total_items_analyzed: number;
    high_relevance_count: number;
    avg_score: number;
    tldr: string;
  };
  themes: Theme[];
  notable_items: {
    id: number;
    title: string;
    url: string | null;
    source: string;
    score: number;
    why_notable: string;
  }[];
  recommendations: {
    priority: "high" | "medium" | "low";
    action: string;
    reasoning: string;
  }[];
  trends: {
    rising: string[];
    falling: string[];
  };
  // AI-powered synthesis
  ai_synthesis?: {
    executive_brief: string;
    key_insight: string;
    action_item: string;
    model_used: string;
  };
}

export async function executeDailyBriefing(
  db: FourDADatabase,
  params: DailyBriefingParams
): Promise<BriefingResult | CompactResult<DailyBriefingKeyData>> {
  const useCompact = params.compact !== false; // Default to compact=true
  const { hours = 24, min_score = 0.5, max_items = 50, format = "full" } = params;

  // Check LLM availability
  const dbInstance = (db as unknown as { db: { prepare: (sql: string) => { get: (...args: unknown[]) => unknown } } }).db;
  const llmConfig = getLLMConfig(dbInstance);
  const shouldSynthesize = params.synthesize ?? canSynthesize(llmConfig);

  // Get relevant items
  const items = db.getRelevantContent(min_score, undefined, max_items, hours);
  const context = db.getUserContext(true, true);

  // Calculate basic stats
  const avgScore = items.length > 0
    ? items.reduce((sum, i) => sum + i.relevance_score, 0) / items.length
    : 0;

  // Extract themes
  const themes = extractThemes(items, context);

  // Identify notable items
  const notableItems = items
    .filter(i => i.relevance_score >= 0.7)
    .slice(0, 5)
    .map(i => ({
      id: i.id,
      title: i.title,
      url: i.url,
      source: i.source_type,
      score: i.relevance_score,
      why_notable: i.relevance_score >= 0.85
        ? "Exceptionally high relevance"
        : "Strong match to your interests",
    }));

  // Generate recommendations
  const recommendations = generateRecommendations(themes, items, context);

  // Get trends (simplified)
  const trends = {
    rising: themes
      .filter(t => t.item_count >= 3)
      .slice(0, 3)
      .map(t => t.name),
    falling: [],
  };

  // Generate TL;DR
  const tldr = generateTldr(items.length, themes, avgScore, notableItems.length);

  const endDate = new Date();
  const startDate = new Date(endDate.getTime() - hours * 60 * 60 * 1000);

  const result: BriefingResult = {
    period: {
      start: startDate.toISOString(),
      end: endDate.toISOString(),
      hours,
    },
    summary: {
      total_items_analyzed: items.length,
      high_relevance_count: items.filter(i => i.relevance_score >= 0.7).length,
      avg_score: Math.round(avgScore * 100) / 100,
      tldr,
    },
    themes: format === "brief" ? themes.slice(0, 3) : themes,
    notable_items: notableItems,
    recommendations: format === "actionable" ? recommendations : recommendations.slice(0, 3),
    trends,
  };

  // AI Synthesis - the actual superpower
  if (shouldSynthesize && canSynthesize(llmConfig)) {
    try {
      const briefingData = {
        period: result.period,
        summary: result.summary,
        themes: result.themes.slice(0, 5),
        notable_items: result.notable_items,
        trends: result.trends,
      };

      const contextData = {
        interests: context.interests.slice(0, 10),
        tech_stack: context.tech_stack,
        role: context.role,
      };

      const synthesis = await synthesize(llmConfig, {
        system: SYNTHESIS_PROMPTS.dailyBriefing.system,
        prompt: SYNTHESIS_PROMPTS.dailyBriefing.buildPrompt(briefingData, contextData),
        max_tokens: 400,
        complexity: SYNTHESIS_PROMPTS.dailyBriefing.complexity,
      });

      // Parse the synthesis into structured output
      const lines = synthesis.synthesis.split("\n").filter(l => l.trim());
      const keyInsight = lines[0] || "";
      const actionItem = lines.find(l => l.toLowerCase().includes("action") || l.toLowerCase().includes("should")) || lines[1] || "";

      result.ai_synthesis = {
        executive_brief: synthesis.synthesis,
        key_insight: keyInsight,
        action_item: actionItem,
        model_used: synthesis.model_used,
      };
    } catch (error) {
      // Synthesis failed - continue without it
      console.error("AI synthesis failed:", error);
    }
  }

  // Return compact or full result based on parameter
  if (useCompact) {
    return createDailyBriefingCompact(result);
  }

  return result;
}

function extractThemes(
  items: { title: string; content: string; relevance_score: number; url: string | null }[],
  context: { interests: { topic: string }[]; tech_stack: string[] }
): Theme[] {
  // Define theme patterns
  const themePatterns: { name: string; keywords: string[] }[] = [
    { name: "AI & Machine Learning", keywords: ["ai", "llm", "gpt", "claude", "machine learning", "neural", "embedding"] },
    { name: "Rust Ecosystem", keywords: ["rust", "cargo", "tokio", "async", "wasm"] },
    { name: "TypeScript & Frontend", keywords: ["typescript", "javascript", "react", "vue", "frontend"] },
    { name: "Databases & Storage", keywords: ["database", "sql", "sqlite", "postgres", "redis", "storage"] },
    { name: "Security", keywords: ["security", "vulnerability", "auth", "encryption", "cve"] },
    { name: "Infrastructure", keywords: ["docker", "kubernetes", "cloud", "aws", "devops"] },
    { name: "Developer Tools", keywords: ["ide", "editor", "cli", "terminal", "git", "testing"] },
  ];

  const themes: Theme[] = [];

  for (const pattern of themePatterns) {
    const matchingItems = items.filter(item => {
      const text = (item.title + " " + item.content).toLowerCase();
      return pattern.keywords.some(kw => text.includes(kw));
    });

    if (matchingItems.length > 0) {
      const avgScore = matchingItems.reduce((sum, i) => sum + i.relevance_score, 0) / matchingItems.length;

      themes.push({
        name: pattern.name,
        item_count: matchingItems.length,
        avg_score: Math.round(avgScore * 100) / 100,
        top_items: matchingItems
          .sort((a, b) => b.relevance_score - a.relevance_score)
          .slice(0, 3)
          .map(i => ({
            title: i.title,
            score: i.relevance_score,
            url: i.url,
          })),
        insight: generateThemeInsight(pattern.name, matchingItems.length, avgScore),
      });
    }
  }

  return themes.sort((a, b) => b.avg_score - a.avg_score);
}

function generateThemeInsight(name: string, count: number, avgScore: number): string {
  if (count >= 5 && avgScore >= 0.7) {
    return `Strong activity in ${name} - this is clearly relevant to your work`;
  } else if (count >= 3) {
    return `Moderate activity in ${name} - worth keeping an eye on`;
  } else {
    return `Some mentions of ${name} - peripheral to your main focus`;
  }
}

function generateRecommendations(
  themes: Theme[],
  items: { relevance_score: number }[],
  context: { interests: { topic: string }[] }
): { priority: "high" | "medium" | "low"; action: string; reasoning: string }[] {
  const recommendations: { priority: "high" | "medium" | "low"; action: string; reasoning: string }[] = [];

  // High relevance items exist
  const highRelevance = items.filter(i => i.relevance_score >= 0.8);
  if (highRelevance.length > 0) {
    recommendations.push({
      priority: "high",
      action: `Review the ${highRelevance.length} high-relevance items`,
      reasoning: "These items scored >80% relevance and directly match your interests",
    });
  }

  // Strong theme detected
  const topTheme = themes[0];
  if (topTheme && topTheme.item_count >= 5) {
    recommendations.push({
      priority: "medium",
      action: `Deep dive into "${topTheme.name}" content`,
      reasoning: `${topTheme.item_count} items cluster around this theme with avg ${Math.round(topTheme.avg_score * 100)}% relevance`,
    });
  }

  // Low overall volume
  if (items.length < 5) {
    recommendations.push({
      priority: "low",
      action: "Check source connectivity",
      reasoning: "Low item count may indicate source fetch issues",
    });
  }

  // Add generic recommendation if empty
  if (recommendations.length === 0) {
    recommendations.push({
      priority: "low",
      action: "Continue monitoring - no urgent actions needed",
      reasoning: "Content flow is normal with no standout items",
    });
  }

  return recommendations;
}

function generateTldr(
  totalItems: number,
  themes: Theme[],
  avgScore: number,
  notableCount: number
): string {
  const parts: string[] = [];

  // Volume
  if (totalItems > 20) {
    parts.push(`Busy period with ${totalItems} relevant items.`);
  } else if (totalItems > 5) {
    parts.push(`Moderate activity with ${totalItems} relevant items.`);
  } else {
    parts.push(`Quiet period with only ${totalItems} relevant items.`);
  }

  // Top theme
  if (themes.length > 0) {
    parts.push(`Top theme: ${themes[0].name} (${themes[0].item_count} items).`);
  }

  // Notable items
  if (notableCount > 0) {
    parts.push(`${notableCount} items scored exceptionally high - review recommended.`);
  }

  // Quality
  if (avgScore >= 0.7) {
    parts.push("Quality is high - good signal-to-noise ratio.");
  } else if (avgScore >= 0.5) {
    parts.push("Decent relevance overall.");
  }

  return parts.join(" ");
}
