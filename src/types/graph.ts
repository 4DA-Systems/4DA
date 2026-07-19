export interface ContentGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  clusters: GraphCluster[];
  meta: GraphMeta;
}

export interface GraphNode {
  id: number;
  title: string;
  url: string | null;
  source_type: string;
  relevance_score: number;
  signal_type: string | null;
  signal_priority: string | null;
  created_at: string;
  primary_topic: string | null;
  cluster_id: string | null;
  /** Total items this node represents (>1 = a story of collapsed near-dupes). */
  member_count: number;
  /** Item ids of all members including the representative. */
  member_ids: number[];
  /** Content category: 'security' | 'release' | 'discussion' | 'research'. */
  category: string;
  /** A member is linked to one of the user's declared dependencies. */
  affects_you: boolean;
  x: number;
  y: number;
}

export interface GraphEdge {
  source: number;
  target: number;
  edge_type: 'semantic' | 'chain' | 'concept' | 'convergence' | 'duplicate';
  weight: number;
  label: string | null;
  methods: string[];
}

export interface GraphCluster {
  id: string;
  label: string;
  node_ids: number[];
  source_count: number;
  /** Mean pairwise embedding cosine among members — theme tightness. */
  coherence: number;
  centroid_x: number;
  centroid_y: number;
}

export interface GraphMeta {
  total_items: number;
  total_edges: number;
  cluster_count: number;
  /** Nodes that represent 2+ collapsed items. */
  story_count: number;
  /** Items folded behind story representatives. */
  collapsed_items: number;
  /** Low-signal isolated items beyond the orbit-ring cap (List view only). */
  hidden_items: number;
  time_window_days: number;
  edge_threshold: string;
  /** Pair-count-weighted mean of per-cluster coherence (null = no clusters). */
  mean_cluster_coherence: number | null;
}
