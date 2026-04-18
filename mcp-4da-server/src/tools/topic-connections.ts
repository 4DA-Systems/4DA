// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Topic Connections Tool
 *
 * Build and explore knowledge graphs from discovered content.
 * This is a SUPERPOWER - it reveals hidden connections between topics.
 *
 * With synthesize=true, uses LLM to explain the significance of connections.
 */

import type { FourDADatabase } from "../db.js";
import { getLLMConfig, canSynthesize, synthesize, SYNTHESIS_PROMPTS } from "../llm.js";
import { createTopicConnectionsCompact, type CompactResult, type TopicConnectionsKeyData } from "../output-manager.js";

export const topicConnectionsTool = {
  name: "topic_connections",
  description: `Discover connections between topics in your content.

With synthesize=true (recommended), uses AI to explain the significance of connections.

Returns:
- Topic co-occurrence graph
- Strongest topic relationships
- Central/hub topics
- Path finding between topics
- AI insights about connections (when enabled)

Use this to understand how your interests connect.`,
  inputSchema: {
    type: "object",
    properties: {
      topics: {
        type: "array",
        items: { type: "string" },
        description: "Specific topics to analyze (optional, auto-detect if not specified)",
      },
      find_path: {
        type: "object",
        properties: {
          from: { type: "string" },
          to: { type: "string" },
        },
        description: "Find connection path between two topics",
      },
      days: {
        type: "number",
        description: "Days of content to analyze (default: 30)",
      },
      synthesize: {
        type: "boolean",
        description: "Use AI to explain connection significance (default: true if LLM configured)",
      },
      compact: {
        type: "boolean",
        description: "Return compact result with file reference (default: true for ~80% token reduction)",
      },
    },
  },
};

export interface TopicConnectionsParams {
  topics?: string[];
  find_path?: { from: string; to: string };
  days?: number;
  synthesize?: boolean;
  compact?: boolean;
}

interface TopicNode {
  topic: string;
  mention_count: number;
  avg_relevance: number;
  centrality: number;
  connections: number;
}

interface TopicEdge {
  from: string;
  to: string;
  co_occurrences: number;
  strength: "strong" | "medium" | "weak";
}

interface TopicPath {
  from: string;
  to: string;
  path: string[];
  direct_connection: boolean;
  connecting_items: { title: string; score: number }[];
}

interface TopicCluster {
  name: string;
  topics: string[];
  item_count: number;
  coherence: number;
}

interface TopicConnectionsResult {
  analysis_period: {
    days: number;
    items_analyzed: number;
  };
  nodes: TopicNode[];
  edges: TopicEdge[];
  path?: TopicPath;
  clusters: TopicCluster[];
  central_topics: string[];
  isolated_topics: string[];
  insights: string[];
  // AI-powered graph interpretation
  ai_insights?: {
    graph_narrative: string;
    surprising_connection: string;
    exploration_suggestion: string;
    hidden_pattern: string;
    model_used: string;
  };
}

export async function executeTopicConnections(
  db: FourDADatabase,
  params: TopicConnectionsParams
): Promise<TopicConnectionsResult | CompactResult<TopicConnectionsKeyData>> {
  const { topics: customTopics, find_path, days = 30 } = params;
  const useCompact = params.compact !== false; // Default to compact=true

  const dbInstance = (db as unknown as { db: { prepare: (sql: string) => { all: (...args: unknown[]) => unknown[]; get: (...args: unknown[]) => unknown } } }).db;

  // Check LLM availability
  const llmConfig = getLLMConfig(dbInstance);
  const shouldSynthesize = params.synthesize ?? canSynthesize(llmConfig);

  // Default topics to analyze
  const topics = customTopics || [
    "rust", "typescript", "python", "go", "javascript",
    "ai", "llm", "embedding", "database", "sql", "sqlite",
    "react", "async", "wasm", "security", "api", "performance"
  ];

  // Get items for analysis
  const items = db.getRelevantContent(0, undefined, 500, days * 24);

  // Build nodes
  const nodes: TopicNode[] = [];
  const topicItems: Map<string, typeof items> = new Map();

  for (const topic of topics) {
    const matching = items.filter(i => {
      const text = (i.title + " " + i.content).toLowerCase();
      return text.includes(topic.toLowerCase());
    });

    if (matching.length > 0) {
      topicItems.set(topic, matching);
      const avgRelevance = matching.reduce((sum, i) => sum + i.relevance_score, 0) / matching.length;

      nodes.push({
        topic,
        mention_count: matching.length,
        avg_relevance: Math.round(avgRelevance * 100) / 100,
        centrality: 0, // Calculated later
        connections: 0, // Calculated later
      });
    }
  }

  // Build edges (co-occurrence)
  const edges: TopicEdge[] = [];
  const nodeTopics = nodes.map(n => n.topic);

  for (let i = 0; i < nodeTopics.length; i++) {
    for (let j = i + 1; j < nodeTopics.length; j++) {
      const topicA = nodeTopics[i];
      const topicB = nodeTopics[j];

      // Count co-occurrences
      const coOccur = items.filter(item => {
        const text = (item.title + " " + item.content).toLowerCase();
        return text.includes(topicA.toLowerCase()) && text.includes(topicB.toLowerCase());
      }).length;

      if (coOccur > 0) {
        edges.push({
          from: topicA,
          to: topicB,
          co_occurrences: coOccur,
          strength: coOccur >= 10 ? "strong" : coOccur >= 3 ? "medium" : "weak",
        });

        // Update connection counts
        const nodeA = nodes.find(n => n.topic === topicA);
        const nodeB = nodes.find(n => n.topic === topicB);
        if (nodeA) nodeA.connections++;
        if (nodeB) nodeB.connections++;
      }
    }
  }

  // Calculate centrality (simplified: connection count weighted by strength)
  const maxConnections = Math.max(...nodes.map(n => n.connections), 1);
  for (const node of nodes) {
    node.centrality = Math.round((node.connections / maxConnections) * 100) / 100;
  }

  // Find path if requested
  let path: TopicPath | undefined;
  if (find_path) {
    path = findTopicPath(find_path.from, find_path.to, edges, items);
  }

  // Identify clusters
  const clusters = identifyClusters(nodes, edges);

  // Central and isolated topics
  const centralTopics = nodes
    .filter(n => n.centrality >= 0.5)
    .sort((a, b) => b.centrality - a.centrality)
    .slice(0, 5)
    .map(n => n.topic);

  const isolatedTopics = nodes
    .filter(n => n.connections === 0)
    .map(n => n.topic);

  // Generate insights
  const insights = generateGraphInsights(nodes, edges, clusters);

  const result: TopicConnectionsResult = {
    analysis_period: {
      days,
      items_analyzed: items.length,
    },
    nodes: nodes.sort((a, b) => b.mention_count - a.mention_count),
    edges: edges.sort((a, b) => b.co_occurrences - a.co_occurrences),
    path,
    clusters,
    central_topics: centralTopics,
    isolated_topics: isolatedTopics,
    insights,
  };

  // AI Graph Insights - the actual superpower
  if (shouldSynthesize && canSynthesize(llmConfig)) {
    try {
      const context = db.getUserContext(true, true);
      const graphData = {
        nodes: result.nodes.slice(0, 15),
        edges: result.edges.slice(0, 20),
        clusters: result.clusters,
        central_topics: result.central_topics,
        isolated_topics: result.isolated_topics,
        path: result.path,
      };

      const contextData = {
        interests: context.interests.slice(0, 10),
        tech_stack: context.tech_stack,
        role: context.role,
      };

      const synthesis = await synthesize(llmConfig, {
        system: SYNTHESIS_PROMPTS.topicConnections.system,
        prompt: SYNTHESIS_PROMPTS.topicConnections.buildPrompt(graphData, contextData),
        max_tokens: 400,
        complexity: SYNTHESIS_PROMPTS.topicConnections.complexity,
      });

      // Parse the synthesis
      const lines = synthesis.synthesis.split("\n").filter(l => l.trim());
      const surprisingConnection = lines.find(l =>
        l.toLowerCase().includes("surprising") ||
        l.toLowerCase().includes("interesting") ||
        l.toLowerCase().includes("unexpected")
      ) || lines[0] || "";
      const exploration = lines.find(l =>
        l.toLowerCase().includes("explore") ||
        l.toLowerCase().includes("investigate") ||
        l.toLowerCase().includes("look into")
      ) || "";
      const hiddenPattern = lines.find(l =>
        l.toLowerCase().includes("pattern") ||
        l.toLowerCase().includes("reveals") ||
        l.toLowerCase().includes("suggests")
      ) || "";

      result.ai_insights = {
        graph_narrative: synthesis.synthesis,
        surprising_connection: surprisingConnection,
        exploration_suggestion: exploration || "Explore connections between central topics",
        hidden_pattern: hiddenPattern || "No hidden patterns detected",
        model_used: synthesis.model_used,
      };
    } catch (error) {
      console.error("AI graph insights failed:", error);
    }
  }

  // Return compact or full result based on parameter
  if (useCompact) {
    return createTopicConnectionsCompact(result);
  }

  return result;
}

function findTopicPath(
  from: string,
  to: string,
  edges: TopicEdge[],
  items: { title: string; content: string; relevance_score: number }[]
): TopicPath {
  // Direct connection
  const directEdge = edges.find(
    e => (e.from === from && e.to === to) || (e.from === to && e.to === from)
  );

  if (directEdge) {
    // Get connecting items
    const connectingItems = items
      .filter(i => {
        const text = (i.title + " " + i.content).toLowerCase();
        return text.includes(from.toLowerCase()) && text.includes(to.toLowerCase());
      })
      .slice(0, 3)
      .map(i => ({ title: i.title, score: i.relevance_score }));

    return {
      from,
      to,
      path: [from, to],
      direct_connection: true,
      connecting_items: connectingItems,
    };
  }

  // Two-hop path (simplified BFS)
  for (const edge of edges) {
    const intermediate = edge.from === from ? edge.to :
                         edge.to === from ? edge.from : null;

    if (intermediate) {
      const secondEdge = edges.find(
        e => (e.from === intermediate && e.to === to) ||
             (e.to === intermediate && e.from === to)
      );

      if (secondEdge) {
        const connectingItems = items
          .filter(i => {
            const text = (i.title + " " + i.content).toLowerCase();
            return text.includes(intermediate.toLowerCase());
          })
          .slice(0, 3)
          .map(i => ({ title: i.title, score: i.relevance_score }));

        return {
          from,
          to,
          path: [from, intermediate, to],
          direct_connection: false,
          connecting_items: connectingItems,
        };
      }
    }
  }

  // No path found
  return {
    from,
    to,
    path: [],
    direct_connection: false,
    connecting_items: [],
  };
}

function identifyClusters(
  nodes: TopicNode[],
  edges: TopicEdge[]
): TopicCluster[] {
  // Predefined cluster patterns
  const clusterPatterns = [
    { name: "Systems Programming", keywords: ["rust", "go", "async", "wasm", "performance"] },
    { name: "AI & ML", keywords: ["ai", "llm", "embedding", "python", "ml"] },
    { name: "Web Development", keywords: ["typescript", "javascript", "react", "frontend", "api"] },
    { name: "Data & Storage", keywords: ["database", "sql", "sqlite", "redis", "postgres"] },
  ];

  const clusters: TopicCluster[] = [];

  for (const pattern of clusterPatterns) {
    const matchingNodes = nodes.filter(n =>
      pattern.keywords.includes(n.topic.toLowerCase())
    );

    if (matchingNodes.length >= 2) {
      // Calculate internal edge count for coherence
      let internalEdges = 0;
      const topics = matchingNodes.map(n => n.topic);
      for (const edge of edges) {
        if (topics.includes(edge.from) && topics.includes(edge.to)) {
          internalEdges++;
        }
      }

      const maxEdges = (topics.length * (topics.length - 1)) / 2;
      const coherence = maxEdges > 0 ? internalEdges / maxEdges : 0;

      clusters.push({
        name: pattern.name,
        topics: matchingNodes.map(n => n.topic),
        item_count: matchingNodes.reduce((sum, n) => sum + n.mention_count, 0),
        coherence: Math.round(coherence * 100) / 100,
      });
    }
  }

  return clusters.sort((a, b) => b.item_count - a.item_count);
}

function generateGraphInsights(
  nodes: TopicNode[],
  edges: TopicEdge[],
  clusters: TopicCluster[]
): string[] {
  const insights: string[] = [];

  // Most central topic
  const mostCentral = nodes.sort((a, b) => b.centrality - a.centrality)[0];
  if (mostCentral) {
    insights.push(
      `"${mostCentral.topic}" is your most connected topic (${mostCentral.connections} connections)`
    );
  }

  // Strongest connection
  const strongestEdge = edges[0];
  if (strongestEdge) {
    insights.push(
      `Strongest connection: "${strongestEdge.from}" ↔ "${strongestEdge.to}" (${strongestEdge.co_occurrences} co-occurrences)`
    );
  }

  // Isolated topics
  const isolated = nodes.filter(n => n.connections === 0);
  if (isolated.length > 0) {
    insights.push(
      `${isolated.length} isolated topic(s): ${isolated.map(n => n.topic).join(", ")} - consider exploring connections`
    );
  }

  // Cluster insight
  const topCluster = clusters[0];
  if (topCluster) {
    insights.push(
      `"${topCluster.name}" is your strongest cluster with ${topCluster.topics.length} related topics`
    );
  }

  return insights;
}
