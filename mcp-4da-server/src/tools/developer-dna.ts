/**
 * developer_dna tool
 *
 * Export a comprehensive Developer DNA profile built from project scans,
 * engagement data, and learned preferences.
 */

import type { FourDADatabase } from "../db.js";

// =============================================================================
// Parameters
// =============================================================================

export interface DeveloperDnaParams {
  include_blind_spots?: boolean;
  max_dependencies?: number;
  max_topics?: number;
}

// =============================================================================
// Tool Definition
// =============================================================================

export const developerDnaTool = {
  name: "developer_dna",
  description: `Export your Developer DNA — a comprehensive profile of your tech identity built from project scans, engagement data, and learned preferences. Includes primary stack, adjacent tech, top dependencies, engaged topics, blind spots, source engagement rates, and aggregate stats.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      include_blind_spots: {
        type: "boolean",
        description: "Include blind spot analysis (dependencies with stale engagement). Default: true",
        default: true,
      },
      max_dependencies: {
        type: "number",
        description: "Maximum dependencies to include. Default: 30",
        default: 30,
      },
      max_topics: {
        type: "number",
        description: "Maximum engaged topics to include. Default: 20",
        default: 20,
      },
    },
  },
};

// =============================================================================
// Row Types (local to this tool)
// =============================================================================

interface TechStackRow {
  technology: string;
}

interface DetectedTechRow {
  name: string;
  category: string;
  confidence: number;
}

interface DependencyRow {
  package_name: string;
  project_path: string;
}

interface InterestRow {
  topic: string;
  weight: number;
}

interface TopicAffinityRow {
  topic: string;
  positive_signals: number;
  negative_signals: number;
  total_exposures: number;
}

interface SourceCountRow {
  source_type: string;
  item_count: number;
}

interface SourceSavedRow {
  source_type: string;
  saved_count: number;
}

interface CountRow {
  cnt: number;
}

interface MinDateRow {
  min_date: string | null;
}

interface ProjectCountRow {
  project_count: number;
}

interface DepCountRow {
  dep_count: number;
}

interface InteractionRow {
  item_id: number;
  action_type: string;
  item_source: string;
  timestamp: string;
}

// =============================================================================
// Result Types
// =============================================================================

interface DependencyEntry {
  name: string;
  project_path: string;
}

interface EngagedTopic {
  topic: string;
  interactions: number;
  percent_of_total: number;
}

interface BlindSpotEntry {
  dependency: string;
  severity: string;
  days_stale: number;
}

interface SourceEngagement {
  source_type: string;
  items_seen: number;
  items_saved: number;
  engagement_rate: number;
}

interface DnaStats {
  total_items_processed: number;
  total_relevant: number;
  rejection_rate: number;
  project_count: number;
  dependency_count: number;
  days_active: number;
}

interface DeveloperDna {
  generated_at: string;
  primary_stack: string[];
  adjacent_tech: string[];
  top_dependencies: DependencyEntry[];
  interests: string[];
  top_engaged_topics: EngagedTopic[];
  blind_spots: BlindSpotEntry[];
  source_engagement: SourceEngagement[];
  stats: DnaStats;
  identity_summary: string;
}

// =============================================================================
// Execution
// =============================================================================

export function executeDeveloperDna(
  db: FourDADatabase,
  params: DeveloperDnaParams,
): DeveloperDna {
  const rawDb = db.getRawDb();
  const includeBlindSpots = params.include_blind_spots ?? true;
  const maxDependencies = params.max_dependencies ?? 30;
  const maxTopics = params.max_topics ?? 20;

  // -------------------------------------------------------------------------
  // 1. Primary Stack — from tech_stack table (user-declared)
  // -------------------------------------------------------------------------
  let primaryStack: string[] = [];
  try {
    const techRows = rawDb
      .prepare("SELECT technology FROM tech_stack ORDER BY technology")
      .all() as TechStackRow[];
    primaryStack = techRows.map((r) => r.technology);
  } catch {
    // table may not exist yet
  }

  // -------------------------------------------------------------------------
  // 2. Adjacent Tech — detected_tech NOT in primary stack
  // -------------------------------------------------------------------------
  let adjacentTech: string[] = [];
  try {
    const detectedRows = rawDb
      .prepare(
        `SELECT DISTINCT name, category, confidence
         FROM detected_tech
         WHERE confidence > 0.3
         ORDER BY confidence DESC
         LIMIT 50`,
      )
      .all() as DetectedTechRow[];

    const primarySet = new Set(primaryStack.map((t) => t.toLowerCase()));
    adjacentTech = detectedRows
      .filter((r) => !primarySet.has(r.name.toLowerCase()))
      .map((r) => r.name);
  } catch {
    // detected_tech table may not exist
  }

  // -------------------------------------------------------------------------
  // 3. Top Dependencies — from project_dependencies table
  // -------------------------------------------------------------------------
  let topDependencies: DependencyEntry[] = [];
  try {
    const depRows = rawDb
      .prepare(
        `SELECT package_name, project_path
         FROM project_dependencies
         ORDER BY package_name
         LIMIT ?`,
      )
      .all(maxDependencies) as DependencyRow[];

    topDependencies = depRows.map((r) => ({
      name: r.package_name,
      project_path: r.project_path,
    }));
  } catch {
    // table may not exist
  }

  // -------------------------------------------------------------------------
  // 4. Interests — from explicit_interests table
  // -------------------------------------------------------------------------
  let interests: string[] = [];
  try {
    const interestRows = rawDb
      .prepare(
        "SELECT topic, weight FROM explicit_interests ORDER BY weight DESC LIMIT 30",
      )
      .all() as InterestRow[];
    interests = interestRows.map((r) => r.topic);
  } catch {
    // table may not exist
  }

  // -------------------------------------------------------------------------
  // 5. Top Engaged Topics — from topic_affinities
  // -------------------------------------------------------------------------
  let topEngagedTopics: EngagedTopic[] = [];
  try {
    const affinityRows = rawDb
      .prepare(
        `SELECT topic, positive_signals, negative_signals, total_exposures
         FROM topic_affinities
         WHERE positive_signals > 0
         ORDER BY positive_signals DESC
         LIMIT ?`,
      )
      .all(maxTopics) as TopicAffinityRow[];

    const totalPositive = affinityRows.reduce(
      (sum, r) => sum + r.positive_signals,
      0,
    );

    topEngagedTopics = affinityRows.map((r) => ({
      topic: r.topic,
      interactions: r.positive_signals,
      percent_of_total:
        totalPositive > 0
          ? Math.round((r.positive_signals / totalPositive) * 10000) / 100
          : 0,
    }));
  } catch {
    // table may not exist
  }

  // -------------------------------------------------------------------------
  // 6. Blind Spots — dependencies with no recent engagement
  // -------------------------------------------------------------------------
  let blindSpots: BlindSpotEntry[] = [];
  if (includeBlindSpots && topDependencies.length > 0) {
    try {
      // Get all interactions for cross-referencing
      const recentInteractions = rawDb
        .prepare(
          `SELECT i.item_id, i.action_type, i.item_source, i.timestamp
           FROM interactions i
           WHERE i.action_type IN ('click', 'save')
           ORDER BY i.timestamp DESC`,
        )
        .all() as InteractionRow[];

      // Build set of recently engaged dependency names
      const now = Date.now();

      for (const dep of topDependencies) {
        const depLower = dep.name.toLowerCase();

        // Check if any interaction item's source mentions this dependency
        // Use a simpler heuristic: check if the dependency name appears in
        // topic_affinities with positive signals
        let lastEngagement: string | null = null;
        try {
          const affinityRow = rawDb
            .prepare(
              `SELECT topic, positive_signals
               FROM topic_affinities
               WHERE LOWER(topic) = ?
               AND positive_signals > 0`,
            )
            .get(depLower) as TopicAffinityRow | undefined;

          if (affinityRow) {
            continue; // Has engagement, not a blind spot
          }
        } catch {
          // Ignore
        }

        // Check for any source_items mentioning this dep that have interactions
        try {
          const mentionedInteraction = rawDb
            .prepare(
              `SELECT i.timestamp
               FROM interactions i
               JOIN source_items si ON i.item_id = si.id AND i.item_source = si.source_type
               WHERE (LOWER(si.title) LIKE ? OR LOWER(si.content) LIKE ?)
               AND i.action_type IN ('click', 'save')
               ORDER BY i.timestamp DESC
               LIMIT 1`,
            )
            .get(`%${depLower}%`, `%${depLower}%`) as
            | { timestamp: string }
            | undefined;

          if (mentionedInteraction) {
            const interactionTime = new Date(
              mentionedInteraction.timestamp,
            ).getTime();
            const daysSince = Math.floor(
              (now - interactionTime) / (1000 * 60 * 60 * 24),
            );

            if (daysSince > 14) {
              blindSpots.push({
                dependency: dep.name,
                severity: daysSince > 60 ? "high" : "medium",
                days_stale: daysSince,
              });
            }
          } else {
            // Never interacted with content about this dependency
            blindSpots.push({
              dependency: dep.name,
              severity: "low",
              days_stale: -1, // indicates "never engaged"
            });
          }
        } catch {
          // Ignore query failures for individual deps
        }
      }

      // Sort by severity
      const severityOrder: Record<string, number> = {
        critical: 4,
        high: 3,
        medium: 2,
        low: 1,
      };
      blindSpots.sort(
        (a, b) =>
          (severityOrder[b.severity] || 0) - (severityOrder[a.severity] || 0),
      );
    } catch {
      // interactions table may not exist
    }
  }

  // -------------------------------------------------------------------------
  // 7. Source Engagement — items seen vs saved per source_type
  // -------------------------------------------------------------------------
  let sourceEngagement: SourceEngagement[] = [];
  try {
    const sourceCounts = rawDb
      .prepare(
        `SELECT source_type, COUNT(*) as item_count
         FROM source_items
         GROUP BY source_type
         ORDER BY item_count DESC`,
      )
      .all() as SourceCountRow[];

    const savedCounts = rawDb
      .prepare(
        `SELECT si.source_type, COUNT(*) as saved_count
         FROM interactions i
         JOIN source_items si ON i.item_id = si.id AND i.item_source = si.source_type
         WHERE i.action_type = 'save'
         GROUP BY si.source_type`,
      )
      .all() as SourceSavedRow[];

    const savedMap = new Map(savedCounts.map((r) => [r.source_type, r.saved_count]));

    sourceEngagement = sourceCounts.map((r) => {
      const saved = savedMap.get(r.source_type) || 0;
      return {
        source_type: r.source_type,
        items_seen: r.item_count,
        items_saved: saved,
        engagement_rate:
          r.item_count > 0
            ? Math.round((saved / r.item_count) * 10000) / 10000
            : 0,
      };
    });
  } catch {
    // tables may not exist
  }

  // -------------------------------------------------------------------------
  // 8. Stats — aggregate numbers
  // -------------------------------------------------------------------------
  let totalItemsProcessed = 0;
  let totalRelevant = 0;
  let projectCount = 0;
  let dependencyCount = 0;
  let daysActive = 0;

  try {
    const totalRow = rawDb
      .prepare("SELECT COUNT(*) as cnt FROM source_items")
      .get() as CountRow;
    totalItemsProcessed = totalRow.cnt;
  } catch {
    // table may not exist
  }

  try {
    // Items with at least one positive interaction = "relevant"
    const relevantRow = rawDb
      .prepare(
        `SELECT COUNT(DISTINCT item_id) as cnt
         FROM interactions
         WHERE action_type IN ('click', 'save')`,
      )
      .get() as CountRow;
    totalRelevant = relevantRow.cnt;
  } catch {
    // table may not exist
  }

  try {
    const projectRow = rawDb
      .prepare(
        "SELECT COUNT(DISTINCT project_path) as project_count FROM project_dependencies",
      )
      .get() as ProjectCountRow;
    projectCount = projectRow.project_count;
  } catch {
    // table may not exist
  }

  try {
    const depRow = rawDb
      .prepare(
        "SELECT COUNT(DISTINCT package_name) as dep_count FROM project_dependencies",
      )
      .get() as DepCountRow;
    dependencyCount = depRow.dep_count;
  } catch {
    // table may not exist
  }

  try {
    const minRow = rawDb
      .prepare("SELECT MIN(created_at) as min_date FROM source_items")
      .get() as MinDateRow;

    if (minRow.min_date) {
      const earliest = new Date(minRow.min_date.replace(" ", "T") + "Z");
      daysActive = Math.max(
        1,
        Math.floor((Date.now() - earliest.getTime()) / (1000 * 60 * 60 * 24)),
      );
    }
  } catch {
    // table may not exist
  }

  const rejectionRate =
    totalItemsProcessed > 0
      ? Math.round(
          ((totalItemsProcessed - totalRelevant) / totalItemsProcessed) * 10000,
        ) / 10000
      : 0;

  const stats: DnaStats = {
    total_items_processed: totalItemsProcessed,
    total_relevant: totalRelevant,
    rejection_rate: rejectionRate,
    project_count: projectCount,
    dependency_count: dependencyCount,
    days_active: daysActive,
  };

  // -------------------------------------------------------------------------
  // 9. Identity Summary — human-readable paragraph
  // -------------------------------------------------------------------------
  const identitySummary = buildIdentitySummary(
    primaryStack,
    adjacentTech,
    interests,
    topEngagedTopics,
    stats,
    sourceEngagement,
  );

  return {
    generated_at: new Date().toISOString(),
    primary_stack: primaryStack,
    adjacent_tech: adjacentTech,
    top_dependencies: topDependencies,
    interests,
    top_engaged_topics: topEngagedTopics,
    blind_spots: blindSpots,
    source_engagement: sourceEngagement,
    stats,
    identity_summary: identitySummary,
  };
}

// =============================================================================
// Helpers
// =============================================================================

function buildIdentitySummary(
  primaryStack: string[],
  adjacentTech: string[],
  interests: string[],
  engagedTopics: EngagedTopic[],
  stats: DnaStats,
  sourceEngagement: SourceEngagement[],
): string {
  const parts: string[] = [];

  // Stack description
  if (primaryStack.length > 0) {
    const stackStr = primaryStack.slice(0, 5).join(", ");
    parts.push(`Primary stack: ${stackStr}${primaryStack.length > 5 ? ` (+${primaryStack.length - 5} more)` : ""}`);
  } else {
    parts.push("No declared tech stack yet");
  }

  // Adjacent discoveries
  if (adjacentTech.length > 0) {
    const adjStr = adjacentTech.slice(0, 3).join(", ");
    parts.push(
      `also working with ${adjStr}${adjacentTech.length > 3 ? ` and ${adjacentTech.length - 3} other detected technologies` : ""}`,
    );
  }

  // Interests
  if (interests.length > 0) {
    const intStr = interests.slice(0, 3).join(", ");
    parts.push(`Key interests: ${intStr}`);
  }

  // Engagement focus
  if (engagedTopics.length > 0) {
    const topTopics = engagedTopics
      .slice(0, 3)
      .map((t) => t.topic)
      .join(", ");
    parts.push(`most engaged with ${topTopics}`);
  }

  // Sources
  if (sourceEngagement.length > 0) {
    const topSource = sourceEngagement[0];
    parts.push(
      `${topSource.source_type} is the primary content source (${topSource.items_seen} items)`,
    );
  }

  // Stats
  if (stats.days_active > 0) {
    parts.push(
      `${stats.total_items_processed} items processed over ${stats.days_active} days across ${stats.project_count} project${stats.project_count !== 1 ? "s" : ""}`,
    );
  }

  return parts.join(". ") + ".";
}
