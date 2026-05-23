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
  centroid_x: number;
  centroid_y: number;
}

export interface GraphMeta {
  total_items: number;
  total_edges: number;
  cluster_count: number;
  time_window_days: number;
  edge_threshold: string;
}
