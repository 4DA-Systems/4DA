// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Source Health Tool
 *
 * Diagnose source fetching, parsing, and data flow issues.
 * This is a SUPERPOWER - it reveals pipeline problems before they become mysteries.
 */

import type { FourDADatabase } from "../db.js";

export const sourceHealthTool = {
  name: "source_health",
  description: `Diagnose the health of 4DA's content sources.
Returns:
- Per-source status (healthy/degraded/failing)
- Recent fetch statistics
- Data quality metrics
- Error patterns detected
- Recommendations for fixes

Use this when content seems stale or sources aren't working.`,
  inputSchema: {
    type: "object",
    properties: {
      source: {
        type: "string",
        description: "Specific source to analyze (optional, all if not specified)",
      },
      hours: {
        type: "number",
        description: "Hours to analyze (default: 24)",
      },
    },
  },
};

export interface SourceHealthParams {
  source?: string;
  hours?: number;
}

interface SourceStatus {
  source: string;
  status: "healthy" | "degraded" | "failing" | "unknown";
  last_item_at: string | null;
  hours_since_last: number | null;
  items_24h: number;
  items_7d: number;
  avg_items_per_day: number;
  quality_metrics: {
    has_url_rate: number;
    has_content_rate: number;
    avg_content_length: number;
  };
  issues: string[];
  recommendations: string[];
}

interface SourceHealthResult {
  analysis_period: {
    start: string;
    end: string;
    hours: number;
  };
  overall_status: "healthy" | "degraded" | "critical";
  sources: SourceStatus[];
  summary: {
    total_sources: number;
    healthy: number;
    degraded: number;
    failing: number;
    total_items_24h: number;
  };
  global_recommendations: string[];
}

export function executeSourceHealth(
  db: FourDADatabase,
  params: SourceHealthParams
): SourceHealthResult {
  const { source, hours = 24 } = params;

  const dbInstance = (db as unknown as { db: { prepare: (sql: string) => { all: (...args: unknown[]) => unknown[]; get: (...args: unknown[]) => unknown } } }).db;

  const endDate = new Date();
  const startDate = new Date(endDate.getTime() - hours * 60 * 60 * 1000);

  // Get all sources or specific one
  let allSources: string[] = [];
  try {
    const sourcesStmt = dbInstance.prepare(`
      SELECT DISTINCT source_type FROM source_items
    `);
    allSources = (sourcesStmt.all() as { source_type: string }[]).map(s => s.source_type);
  } catch {
    // source_items table may not exist in standalone mode
  }

  if (allSources.length === 0 && !source) {
    return {
      analysis_period: {
        start: startDate.toISOString(),
        end: endDate.toISOString(),
        hours,
      },
      overall_status: "healthy" as const,
      sources: [],
      summary: {
        total_sources: 0,
        healthy: 0,
        degraded: 0,
        failing: 0,
        total_items_24h: 0,
      },
      global_recommendations: [
        "No content sources found. Install the 4DA desktop app to ingest content from Hacker News, GitHub, arXiv, and more.",
        "Vulnerability scanning works independently — try the vulnerability_scan tool.",
      ],
    };
  }

  const sourcesToAnalyze = source ? [source] : allSources;

  const sourceStatuses: SourceStatus[] = [];

  for (const src of sourcesToAnalyze) {
    const status = analyzeSource(dbInstance, src, hours);
    sourceStatuses.push(status);
  }

  // Calculate summary
  const healthy = sourceStatuses.filter(s => s.status === "healthy").length;
  const degraded = sourceStatuses.filter(s => s.status === "degraded").length;
  const failing = sourceStatuses.filter(s => s.status === "failing").length;
  const totalItems24h = sourceStatuses.reduce((sum, s) => sum + s.items_24h, 0);

  // Determine overall status
  const overallStatus: "healthy" | "degraded" | "critical" =
    failing > 0 ? "critical" :
    degraded > 0 ? "degraded" : "healthy";

  // Global recommendations
  const globalRecs: string[] = [];
  if (failing > 0) {
    globalRecs.push(`${failing} source(s) failing - check network connectivity and API status`);
  }
  if (totalItems24h === 0) {
    globalRecs.push("No items in last 24h - verify background jobs are running");
  }
  if (degraded > healthy) {
    globalRecs.push("Most sources degraded - consider reviewing fetch intervals");
  }

  return {
    analysis_period: {
      start: startDate.toISOString(),
      end: endDate.toISOString(),
      hours,
    },
    overall_status: overallStatus,
    sources: sourceStatuses,
    summary: {
      total_sources: sourceStatuses.length,
      healthy,
      degraded,
      failing,
      total_items_24h: totalItems24h,
    },
    global_recommendations: globalRecs,
  };
}

function analyzeSource(
  db: { prepare: (sql: string) => { all: (...args: unknown[]) => unknown[]; get: (...args: unknown[]) => unknown } },
  source: string,
  hours: number
): SourceStatus {
  // Last item
  const lastItemStmt = db.prepare(`
    SELECT created_at FROM source_items
    WHERE source_type = ?
    ORDER BY created_at DESC
    LIMIT 1
  `);
  const lastItem = lastItemStmt.get(source) as { created_at: string } | undefined;

  const lastItemAt = lastItem?.created_at || null;
  const hoursSinceLast = lastItemAt
    ? (Date.now() - new Date(lastItemAt.replace(" ", "T") + "Z").getTime()) / (1000 * 60 * 60)
    : null;

  // Items in last 24h
  const items24hStmt = db.prepare(`
    SELECT COUNT(*) as count FROM source_items
    WHERE source_type = ?
    AND datetime(created_at) >= datetime('now', '-24 hours')
  `);
  const items24h = (items24hStmt.get(source) as { count: number })?.count || 0;

  // Items in last 7d
  const items7dStmt = db.prepare(`
    SELECT COUNT(*) as count FROM source_items
    WHERE source_type = ?
    AND datetime(created_at) >= datetime('now', '-7 days')
  `);
  const items7d = (items7dStmt.get(source) as { count: number })?.count || 0;

  // Quality metrics
  const qualityStmt = db.prepare(`
    SELECT
      AVG(CASE WHEN url IS NOT NULL AND url != '' THEN 1.0 ELSE 0.0 END) as has_url_rate,
      AVG(CASE WHEN content IS NOT NULL AND content != '' THEN 1.0 ELSE 0.0 END) as has_content_rate,
      AVG(LENGTH(COALESCE(content, ''))) as avg_content_length
    FROM source_items
    WHERE source_type = ?
    AND datetime(created_at) >= datetime('now', '-7 days')
  `);
  const quality = qualityStmt.get(source) as {
    has_url_rate: number;
    has_content_rate: number;
    avg_content_length: number;
  } | undefined;

  // Determine issues
  const issues: string[] = [];
  const recommendations: string[] = [];

  // Check for staleness
  if (hoursSinceLast !== null && hoursSinceLast > 24) {
    issues.push(`No new items in ${Math.round(hoursSinceLast)} hours`);
    recommendations.push("Check if source is enabled and API is accessible");
  }

  // Check for low volume
  const avgPerDay = items7d / 7;
  if (avgPerDay < 1 && items7d > 0) {
    issues.push(`Very low volume: ${avgPerDay.toFixed(1)} items/day average`);
    recommendations.push("Source may have fetch issues or very restrictive filters");
  }

  // Check quality
  if (quality && quality.has_url_rate < 0.8) {
    issues.push(`${Math.round((1 - quality.has_url_rate) * 100)}% of items missing URLs`);
  }
  if (quality && quality.has_content_rate < 0.5) {
    issues.push(`${Math.round((1 - quality.has_content_rate) * 100)}% of items missing content`);
    recommendations.push("Content extraction may be failing - check parser");
  }

  // Determine status
  let status: "healthy" | "degraded" | "failing" | "unknown";
  if (items7d === 0) {
    status = "unknown";
  } else if (hoursSinceLast !== null && hoursSinceLast > 48) {
    status = "failing";
  } else if (issues.length > 2 || (hoursSinceLast !== null && hoursSinceLast > 24)) {
    status = "degraded";
  } else {
    status = "healthy";
  }

  return {
    source,
    status,
    last_item_at: lastItemAt,
    hours_since_last: hoursSinceLast ? Math.round(hoursSinceLast * 10) / 10 : null,
    items_24h: items24h,
    items_7d: items7d,
    avg_items_per_day: Math.round(avgPerDay * 10) / 10,
    quality_metrics: {
      has_url_rate: Math.round((quality?.has_url_rate || 0) * 100) / 100,
      has_content_rate: Math.round((quality?.has_content_rate || 0) * 100) / 100,
      avg_content_length: Math.round(quality?.avg_content_length || 0),
    },
    issues,
    recommendations,
  };
}
