// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Common shared types

import type { SourceRelevance } from './analysis';

export interface ContextFile {
  path: string;
  content: string;
  lines: number;
}

export interface AppState {
  contextFiles: ContextFile[];
  relevanceResults: SourceRelevance[];
  status: string;
  loading: boolean;
  analysisComplete: boolean;
  progress: number;
  progressMessage: string;
  progressStage: string;
  lastAnalyzedAt: Date | null;
}

export type FeedbackAction = 'save' | 'dismiss' | 'mark_irrelevant' | 'click' | 'snooze';
export type FeedbackGiven = Record<number, FeedbackAction>;

// Void Engine types
export interface VoidSignal {
  pulse: number;             // 0-1: source fetch activity
  heat: number;              // 0-1: avg relevance from last analysis
  burst: number;             // 0-1: discovery flash (score > 0.7)
  morph: number;             // 0-1: ACE file change activity
  error: number;             // 0-1: recent error indicator
  staleness: number;         // 0-1: hours since last analysis / 24
  item_count: number;        // total cached items
  signal_intensity: number;  // 0-1: highest signal priority / 4
  signal_urgency: number;    // 0-1: weighted urgency from signal types
  critical_count: number;    // count of critical-priority signals
  signal_color_shift: number; // -1 to +1: cool (learning) to warm (alert)
  metabolism: number;         // 0-1: intelligence metabolism health (autophagy calibration)
  open_windows: number;       // count of open decision windows
  advantage_trend: number;    // -1 to +1: compound advantage declining to growing
}

export interface SuggestedInterest {
  topic: string;
  source: string;
  confidence: number;
  already_declared: boolean;
}

export interface IndexedStats {
  total_documents: number;
  total_chunks: number;
  total_words: number;
  by_type: Array<{ file_type: string; count: number }>;
}
