/**
 * Tools: record_metric, get_metrics, get_quality_report
 */

import type { ToolEntry, ToolContext, ToolResponse } from "../types.js";

function handleRecordMetric(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { metric_type, value, context, task_id, session_id } = args as {
    metric_type: string;
    value: number;
    context?: string;
    task_id?: string;
    session_id?: string;
  };

  ctx.db
    .prepare(
      `INSERT INTO quality_metrics (metric_type, value, context, task_id, session_id)
       VALUES (?, ?, ?, ?, ?)`
    )
    .run(metric_type, value, context || null, task_id || null, session_id || null);

  return {
    content: [
      {
        type: "text",
        text: `Metric '${metric_type}' recorded with value ${value}.`,
      },
    ],
  };
}

function handleGetMetrics(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { metric_type, since, limit = 100 } = args as {
    metric_type?: string;
    since?: string;
    limit?: number;
  };

  let results;
  if (since) {
    results = ctx.db
      .prepare(
        `SELECT * FROM quality_metrics
         WHERE timestamp >= ?
         ORDER BY timestamp DESC`
      )
      .all(since);
    if (metric_type) {
      results = (results as Array<{ metric_type: string }>).filter((r) =>
        r.metric_type.includes(metric_type)
      );
    }
    results = (results as unknown[]).slice(0, limit);
  } else {
    const pattern = metric_type ? `%${metric_type}%` : "%";
    results = ctx.db
      .prepare(
        `SELECT * FROM quality_metrics
         WHERE metric_type LIKE ?
         ORDER BY timestamp DESC
         LIMIT ?`
      )
      .all(pattern, limit);
  }

  return {
    content: [
      {
        type: "text",
        text:
          (results as unknown[]).length > 0
            ? JSON.stringify(results, null, 2)
            : "No metrics found.",
      },
    ],
  };
}

function handleGetQualityReport(
  args: Record<string, unknown>,
  ctx: ToolContext
): ToolResponse {
  const { since } = args as { since?: string };

  // Default to 7 days ago
  const sinceDate =
    since || new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString();

  const aggregates = ctx.db
    .prepare(
      `SELECT
         metric_type,
         COUNT(*) as count,
         AVG(value) as avg_value,
         MIN(value) as min_value,
         MAX(value) as max_value,
         SUM(value) as sum_value
       FROM quality_metrics
       WHERE timestamp >= ?
       GROUP BY metric_type`
    )
    .all(sinceDate) as Array<{
    metric_type: string;
    count: number;
    avg_value: number;
    min_value: number;
    max_value: number;
    sum_value: number;
  }>;

  // Build report
  const report: Record<string, unknown> = {
    period_start: sinceDate,
    period_end: new Date().toISOString(),
    metrics: {},
  };

  for (const agg of aggregates) {
    (report.metrics as Record<string, unknown>)[agg.metric_type] = {
      count: agg.count,
      average: Math.round(agg.avg_value * 100) / 100,
      min: agg.min_value,
      max: agg.max_value,
      total: agg.sum_value,
    };
  }

  // Add summary interpretations
  const metrics = report.metrics as Record<
    string,
    { count: number; average: number }
  >;
  const summary: string[] = [];

  if (metrics.rework) {
    const reworkRate = metrics.rework.average;
    summary.push(
      `Rework rate: ${(reworkRate * 100).toFixed(1)}% (target: <20%)`
    );
  }

  if (metrics.gate_pass && metrics.gate_fail) {
    const passRate =
      metrics.gate_pass.count /
      (metrics.gate_pass.count + metrics.gate_fail.count);
    summary.push(
      `Gate pass rate: ${(passRate * 100).toFixed(1)}% (target: >70%)`
    );
  }

  if (metrics.iteration_count) {
    summary.push(
      `Mean iterations: ${metrics.iteration_count.average.toFixed(1)} (target: <5)`
    );
  }

  report.summary = summary;

  return {
    content: [
      {
        type: "text",
        text: JSON.stringify(report, null, 2),
      },
    ],
  };
}

export const metricTools: ToolEntry[] = [
  {
    definition: {
      name: "record_metric",
      description:
        "Record a quality metric for CADE tracking. Use to track rework, iterations, gate passes, etc.",
      inputSchema: {
        type: "object",
        properties: {
          metric_type: {
            type: "string",
            description:
              "Type of metric: 'rework', 'iteration_count', 'gate_pass', 'gate_fail', 'confidence', 'task_complete'",
          },
          value: {
            type: "number",
            description: "Numeric value for the metric",
          },
          context: {
            type: "string",
            description: "Context about what generated this metric",
          },
          task_id: {
            type: "string",
            description: "Optional task identifier",
          },
          session_id: {
            type: "string",
            description: "Optional session identifier",
          },
        },
        required: ["metric_type", "value"],
      },
    },
    handler: handleRecordMetric,
  },
  {
    definition: {
      name: "get_metrics",
      description: "Retrieve quality metrics with optional filtering.",
      inputSchema: {
        type: "object",
        properties: {
          metric_type: {
            type: "string",
            description:
              "Filter by metric type (optional, supports wildcards)",
          },
          since: {
            type: "string",
            description:
              "ISO date string to get metrics since (e.g., '2024-01-01')",
          },
          limit: {
            type: "number",
            description: "Maximum results to return (default: 100)",
          },
        },
      },
    },
    handler: handleGetMetrics,
  },
  {
    definition: {
      name: "get_quality_report",
      description:
        "Generate a summary quality report showing aggregated metrics and trends.",
      inputSchema: {
        type: "object",
        properties: {
          since: {
            type: "string",
            description:
              "ISO date string for report period start (default: 7 days ago)",
          },
        },
      },
    },
    handler: handleGetQualityReport,
  },
];
