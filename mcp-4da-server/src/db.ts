/**
 * Database module for 4DA MCP Server
 *
 * Read-only access to the 4DA SQLite database.
 * Only the record_feedback function performs writes.
 */

import Database from "better-sqlite3";
import path from "path";
import * as fs from "fs";
import * as os from "os";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
import type {
  SourceItem,
  RelevantItem,
  Interest,
  DetectedTech,
  ActiveTopic,
  TopicAffinity,
  AntiTopic,
  UserContext,
  ScoreBreakdown,
  MatchingContext,
  RelevanceExplanation,
  FeedbackAction,
  FeedbackResult,
} from "./types.js";

/**
 * Resolve the database path by checking multiple locations in priority order:
 * 1. FOURDA_DB_PATH env var
 * 2. data/4da.db relative to cwd (development)
 * 3. data/4da.db relative to project root (mcp-4da-server is inside project root)
 * 4. Platform-specific Tauri app data dirs (production)
 * 5. Final fallback: data/4da.db relative to cwd
 */
function getDefaultDbPath(): string {
  // 1. Environment variable (highest priority)
  if (process.env.FOURDA_DB_PATH) {
    return process.env.FOURDA_DB_PATH;
  }

  // 2. Relative to cwd (development)
  const cwdPath = path.resolve(process.cwd(), "data", "4da.db");
  if (fs.existsSync(cwdPath)) {
    return cwdPath;
  }

  // 3. Relative to project root (mcp-4da-server is inside project root)
  const projectRootPath = path.resolve(__dirname, "..", "..", "data", "4da.db");
  if (fs.existsSync(projectRootPath)) {
    return projectRootPath;
  }

  // 4. Platform-specific Tauri app data dirs (production)
  const platform = process.platform;
  let appDataPath: string;
  if (platform === "win32") {
    appDataPath = path.join(
      process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming"),
      "com.4da.app",
      "data",
      "4da.db"
    );
  } else if (platform === "darwin") {
    appDataPath = path.join(
      os.homedir(),
      "Library",
      "Application Support",
      "com.4da.app",
      "data",
      "4da.db"
    );
  } else {
    appDataPath = path.join(
      os.homedir(),
      ".local",
      "share",
      "com.4da.app",
      "data",
      "4da.db"
    );
  }
  if (fs.existsSync(appDataPath)) {
    return appDataPath;
  }

  // 5. Final fallback
  return path.resolve(process.cwd(), "data", "4da.db");
}

/**
 * 4DA Database accessor
 */
export class FourDADatabase {
  private db: Database.Database;

  constructor(dbPath?: string) {
    const resolvedPath = dbPath || getDefaultDbPath();

    // Resolve path - if relative, resolve from cwd
    const absolutePath = path.isAbsolute(resolvedPath)
      ? resolvedPath
      : path.resolve(process.cwd(), resolvedPath);

    try {
      this.db = new Database(absolutePath, { readonly: false }); // Need write for feedback
      this.db.pragma("journal_mode = WAL");
    } catch (error) {
      throw new Error(
        `Failed to open 4DA database at ${absolutePath}: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }

  /**
   * Close the database connection
   */
  close(): void {
    this.db.close();
  }

  // ===========================================================================
  // Source Items
  // ===========================================================================

  /**
   * Get relevant content items with computed relevance scores
   */
  getRelevantContent(
    minScore: number = 0.35,
    sourceType?: string,
    limit: number = 20,
    sinceHours: number = 24
  ): RelevantItem[] {
    const sinceDate = new Date(Date.now() - sinceHours * 60 * 60 * 1000)
      .toISOString()
      .replace("T", " ")
      .slice(0, 19);

    // Get user context for scoring
    const context = this.getUserContext(true, true);

    // Build query - get recent items
    let query = `
      SELECT id, source_type, source_id, url, title, content, content_hash, created_at, last_seen
      FROM source_items
      WHERE datetime(created_at) >= datetime(?)
    `;
    const params: (string | number)[] = [sinceDate];

    if (sourceType) {
      query += ` AND source_type = ?`;
      params.push(sourceType);
    }

    query += ` ORDER BY created_at DESC LIMIT ?`;
    params.push(limit * 5); // Get more to filter

    const stmt = this.db.prepare(query);
    const items = stmt.all(...params) as SourceItem[];

    // Compute relevance scores and filter
    const scoredItems: RelevantItem[] = [];
    const now = Date.now();

    for (const item of items) {
      const score = this.computeRelevanceScore(item, context);

      if (score >= minScore) {
        const createdAt = new Date(item.created_at.replace(" ", "T") + "Z");
        const hoursAgo = Math.round((now - createdAt.getTime()) / (1000 * 60 * 60));
        const discoveredAgo =
          hoursAgo < 1
            ? "< 1 hour ago"
            : hoursAgo < 24
              ? `${hoursAgo} hours ago`
              : `${Math.round(hoursAgo / 24)} days ago`;

        scoredItems.push({
          id: item.id,
          source_type: item.source_type,
          source_id: item.source_id,
          url: item.url,
          title: item.title,
          content: item.content.substring(0, 500), // Truncate for readability
          relevance_score: Math.round(score * 100) / 100,
          created_at: item.created_at,
          discovered_ago: discoveredAgo,
        });
      }
    }

    // Sort by relevance and limit
    return scoredItems
      .sort((a, b) => b.relevance_score - a.relevance_score)
      .slice(0, limit);
  }

  /**
   * Get a single source item by ID and type
   */
  getSourceItem(itemId: number, sourceType: string): SourceItem | null {
    const stmt = this.db.prepare(`
      SELECT id, source_type, source_id, url, title, content, content_hash, created_at, last_seen
      FROM source_items
      WHERE id = ? AND source_type = ?
    `);
    return (stmt.get(itemId, sourceType) as SourceItem) || null;
  }

  // ===========================================================================
  // User Context
  // ===========================================================================

  /**
   * Get the user's context (what 4DA knows about them)
   */
  getUserContext(includeAce: boolean = true, includeLearned: boolean = true): UserContext {
    // Static identity
    const roleStmt = this.db.prepare("SELECT role FROM user_identity WHERE id = 1");
    const roleRow = roleStmt.get() as { role: string | null } | undefined;

    const techStmt = this.db.prepare("SELECT technology FROM tech_stack ORDER BY technology");
    const tech = techStmt.all() as { technology: string }[];

    const domainsStmt = this.db.prepare("SELECT domain FROM domains ORDER BY domain");
    const domains = domainsStmt.all() as { domain: string }[];

    const interestsStmt = this.db.prepare(`
      SELECT id, topic, weight, source FROM explicit_interests ORDER BY weight DESC
    `);
    const interests = interestsStmt.all() as Interest[];

    const exclusionsStmt = this.db.prepare("SELECT topic FROM exclusions ORDER BY topic");
    const exclusions = exclusionsStmt.all() as { topic: string }[];

    const context: UserContext = {
      role: roleRow?.role || null,
      tech_stack: tech.map((t) => t.technology),
      domains: domains.map((d) => d.domain),
      interests: interests,
      exclusions: exclusions.map((e) => e.topic),
    };

    // ACE-detected context
    if (includeAce) {
      try {
        const detectedTechStmt = this.db.prepare(`
          SELECT name, category, confidence, source
          FROM detected_tech
          WHERE confidence > 0.3
          ORDER BY confidence DESC
          LIMIT 50
        `);
        const detectedTech = detectedTechStmt.all() as DetectedTech[];

        const activeTopicsStmt = this.db.prepare(`
          SELECT topic, weight, confidence, source, last_seen
          FROM active_topics
          WHERE confidence > 0.3
          ORDER BY weight DESC
          LIMIT 50
        `);
        const activeTopics = activeTopicsStmt.all() as ActiveTopic[];

        context.ace = {
          detected_tech: detectedTech,
          active_topics: activeTopics,
        };
      } catch {
        // ACE tables might not exist
        context.ace = {
          detected_tech: [],
          active_topics: [],
        };
      }
    }

    // Learned preferences
    if (includeLearned) {
      try {
        const affinitiesStmt = this.db.prepare(`
          SELECT topic, affinity_score, confidence, positive_signals, negative_signals, total_exposures
          FROM topic_affinities
          WHERE confidence > 0.1
          ORDER BY affinity_score DESC
          LIMIT 50
        `);
        const affinities = affinitiesStmt.all() as TopicAffinity[];

        const antiTopicsStmt = this.db.prepare(`
          SELECT topic, rejection_count, confidence, auto_detected, user_confirmed
          FROM anti_topics
          WHERE confidence > 0.3
          ORDER BY confidence DESC
          LIMIT 50
        `);
        const antiTopics = antiTopicsStmt.all() as AntiTopic[];

        context.learned = {
          topic_affinities: affinities,
          anti_topics: antiTopics.map((at) => ({
            ...at,
            auto_detected: Boolean(at.auto_detected),
            user_confirmed: Boolean(at.user_confirmed),
          })),
        };
      } catch {
        // Learned tables might not exist
        context.learned = {
          topic_affinities: [],
          anti_topics: [],
        };
      }
    }

    return context;
  }

  // ===========================================================================
  // Relevance Explanation
  // ===========================================================================

  /**
   * Explain why an item was considered relevant
   */
  explainRelevance(itemId: number, sourceType: string): RelevanceExplanation | null {
    const item = this.getSourceItem(itemId, sourceType);
    if (!item) {
      return null;
    }

    const context = this.getUserContext(true, true);
    const itemTopics = this.extractTopics(item.title + " " + item.content);

    // Compute matching elements
    const matchingInterests = context.interests
      .filter((i) => itemTopics.some((t) => this.topicMatches(t, i.topic)))
      .map((i) => i.topic);

    const matchingTech = context.tech_stack.filter((tech) =>
      itemTopics.some((t) => this.topicMatches(t, tech))
    );

    const matchingTopics =
      context.ace?.active_topics
        .filter((at) => itemTopics.some((t) => this.topicMatches(t, at.topic)))
        .map((at) => at.topic) || [];

    const matchingAffinities =
      context.learned?.topic_affinities
        .filter(
          (ta) =>
            ta.affinity_score > 0 && itemTopics.some((t) => this.topicMatches(t, ta.topic))
        )
        .map((ta) => ta.topic) || [];

    // Compute score breakdown
    const staticScore =
      matchingInterests.length * 0.3 +
      matchingTech.length * 0.2 +
      (context.domains.some((d) => itemTopics.some((t) => this.topicMatches(t, d)))
        ? 0.15
        : 0);

    const aceScore = context.ace
      ? matchingTopics.length * 0.1 +
        context.ace.detected_tech.filter((dt) =>
          itemTopics.some((t) => this.topicMatches(t, dt.name))
        ).length *
          0.05
      : 0;

    const learnedScore = context.learned
      ? context.learned.topic_affinities
          .filter(
            (ta) =>
              ta.affinity_score > 0 && itemTopics.some((t) => this.topicMatches(t, ta.topic))
          )
          .reduce((sum, ta) => sum + ta.affinity_score * 0.1 * ta.confidence, 0)
      : 0;

    // Anti-penalty from exclusions and anti-topics
    let antiPenalty = 0;
    for (const exclusion of context.exclusions) {
      if (itemTopics.some((t) => this.topicMatches(t, exclusion))) {
        antiPenalty = 1.0; // Hard exclusion
        break;
      }
    }
    if (antiPenalty === 0 && context.learned) {
      for (const antiTopic of context.learned.anti_topics) {
        if (itemTopics.some((t) => this.topicMatches(t, antiTopic.topic))) {
          antiPenalty = Math.max(antiPenalty, antiTopic.confidence * 0.5);
        }
      }
    }

    const finalScore = Math.max(0, Math.min(1, staticScore + aceScore + learnedScore - antiPenalty));

    const scoreBreakdown: ScoreBreakdown = {
      embedding_similarity: null, // Not computing embedding similarity in MCP server
      static_match_score: Math.round(staticScore * 100) / 100,
      ace_match_score: Math.round(aceScore * 100) / 100,
      learned_affinity_score: Math.round(learnedScore * 100) / 100,
      anti_penalty: Math.round(antiPenalty * 100) / 100,
      final_score: Math.round(finalScore * 100) / 100,
    };

    const matchingContext: MatchingContext = {
      matching_interests: matchingInterests,
      matching_tech: matchingTech,
      matching_topics: matchingTopics,
      matching_affinities: matchingAffinities,
    };

    // Generate human-readable explanation
    const explanationParts: string[] = [];

    if (matchingInterests.length > 0) {
      explanationParts.push(
        `Matches your interests: ${matchingInterests.slice(0, 3).join(", ")}`
      );
    }
    if (matchingTech.length > 0) {
      explanationParts.push(`Related to your tech stack: ${matchingTech.slice(0, 3).join(", ")}`);
    }
    if (matchingTopics.length > 0) {
      explanationParts.push(
        `Relevant to recent work: ${matchingTopics.slice(0, 3).join(", ")}`
      );
    }
    if (matchingAffinities.length > 0) {
      explanationParts.push(
        `You've shown interest in: ${matchingAffinities.slice(0, 3).join(", ")}`
      );
    }
    if (antiPenalty > 0) {
      explanationParts.push(`Score reduced due to excluded topics`);
    }

    const explanation =
      explanationParts.length > 0
        ? explanationParts.join(". ") + "."
        : "No specific matches found - may be based on general relevance.";

    return {
      item_id: itemId,
      source_type: sourceType,
      title: item.title,
      score_breakdown: scoreBreakdown,
      matching_context: matchingContext,
      explanation,
    };
  }

  // ===========================================================================
  // Feedback
  // ===========================================================================

  /**
   * Record user feedback on an item
   */
  recordFeedback(
    itemId: number,
    sourceType: string,
    action: FeedbackAction
  ): FeedbackResult {
    // Verify item exists
    const item = this.getSourceItem(itemId, sourceType);
    if (!item) {
      return {
        success: false,
        message: `Item ${itemId} of type ${sourceType} not found`,
      };
    }

    // Map action to signal strength
    const signalStrength: Record<FeedbackAction, number> = {
      click: 0.3,
      save: 0.8,
      dismiss: -0.2,
      mark_irrelevant: -0.5,
    };

    try {
      const stmt = this.db.prepare(`
        INSERT INTO interactions (item_id, action_type, item_source, signal_strength, timestamp)
        VALUES (?, ?, ?, ?, datetime('now'))
      `);

      const result = stmt.run(itemId, action, sourceType, signalStrength[action]);

      return {
        success: true,
        message: `Recorded ${action} feedback for item ${itemId}`,
        interaction_id: result.lastInsertRowid as number,
      };
    } catch (error) {
      return {
        success: false,
        message: `Failed to record feedback: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }

  // ===========================================================================
  // Helper Methods
  // ===========================================================================

  /**
   * Compute relevance score for an item based on user context
   */
  private computeRelevanceScore(item: SourceItem, context: UserContext): number {
    const itemTopics = this.extractTopics(item.title + " " + item.content);

    // Check hard exclusions first
    for (const exclusion of context.exclusions) {
      if (itemTopics.some((t) => this.topicMatches(t, exclusion))) {
        return 0;
      }
    }

    let score = 0;

    // Static identity matching
    for (const interest of context.interests) {
      if (itemTopics.some((t) => this.topicMatches(t, interest.topic))) {
        score += 0.3 * interest.weight;
      }
    }

    for (const tech of context.tech_stack) {
      if (itemTopics.some((t) => this.topicMatches(t, tech))) {
        score += 0.2;
      }
    }

    for (const domain of context.domains) {
      if (itemTopics.some((t) => this.topicMatches(t, domain))) {
        score += 0.15;
      }
    }

    // ACE-detected context
    if (context.ace) {
      for (const topic of context.ace.active_topics) {
        if (itemTopics.some((t) => this.topicMatches(t, topic.topic))) {
          score += 0.1 * topic.weight * topic.confidence;
        }
      }

      for (const tech of context.ace.detected_tech) {
        if (itemTopics.some((t) => this.topicMatches(t, tech.name))) {
          score += 0.05 * tech.confidence;
        }
      }
    }

    // Learned affinities
    if (context.learned) {
      for (const affinity of context.learned.topic_affinities) {
        if (
          affinity.affinity_score > 0 &&
          itemTopics.some((t) => this.topicMatches(t, affinity.topic))
        ) {
          score += affinity.affinity_score * 0.1 * affinity.confidence;
        }
      }

      // Anti-topics penalty
      for (const antiTopic of context.learned.anti_topics) {
        if (itemTopics.some((t) => this.topicMatches(t, antiTopic.topic))) {
          score -= antiTopic.confidence * 0.3;
        }
      }
    }

    return Math.max(0, Math.min(1, score));
  }

  /**
   * Extract topics from text (simple keyword extraction)
   */
  private extractTopics(text: string): string[] {
    // Simple topic extraction - split on word boundaries, filter meaningful words
    return text
      .toLowerCase()
      .split(/[\s\-_.,;:!?'"()\[\]{}]+/)
      .filter((word) => word.length > 2)
      .filter((word) => !this.isStopWord(word));
  }

  /**
   * Check if a topic matches a term (case-insensitive, partial match)
   */
  private topicMatches(topic: string, term: string): boolean {
    const normalizedTopic = topic.toLowerCase();
    const normalizedTerm = term.toLowerCase();
    return (
      normalizedTopic.includes(normalizedTerm) || normalizedTerm.includes(normalizedTopic)
    );
  }

  /**
   * Check if a word is a stop word
   */
  private isStopWord(word: string): boolean {
    const stopWords = new Set([
      "the",
      "a",
      "an",
      "and",
      "or",
      "but",
      "in",
      "on",
      "at",
      "to",
      "for",
      "of",
      "with",
      "by",
      "from",
      "as",
      "is",
      "was",
      "are",
      "were",
      "been",
      "be",
      "have",
      "has",
      "had",
      "do",
      "does",
      "did",
      "will",
      "would",
      "could",
      "should",
      "may",
      "might",
      "can",
      "this",
      "that",
      "these",
      "those",
      "it",
      "its",
      "they",
      "them",
      "their",
      "we",
      "us",
      "our",
      "you",
      "your",
      "he",
      "she",
      "him",
      "her",
      "his",
      "hers",
      "who",
      "what",
      "when",
      "where",
      "why",
      "how",
      "all",
      "each",
      "every",
      "both",
      "few",
      "more",
      "most",
      "other",
      "some",
      "such",
      "no",
      "not",
      "only",
      "same",
      "so",
      "than",
      "too",
      "very",
      "just",
      "also",
      "now",
      "here",
      "there",
      "then",
      "new",
      "first",
      "one",
      "two",
    ]);
    return stopWords.has(word);
  }
}

/**
 * Create a database instance
 */
export function createDatabase(dbPath?: string): FourDADatabase {
  return new FourDADatabase(dbPath);
}
