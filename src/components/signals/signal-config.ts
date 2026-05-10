// SPDX-License-Identifier: FSL-1.1-Apache-2.0

// ============================================================================
// Signal & Priority display configuration
// ============================================================================

export interface SignalStyleConfig {
  icon: string;
  color: string;
  borderColor: string;
  bgColor: string;
}

export interface PriorityStyleConfig {
  label: string;
  color: string;
  bgColor: string;
  dot: string;
}

export interface LaneConfig {
  key: string;
  label: string;
  icon: string;
  color: string;
  borderColor: string;
  types: Set<string>;
  priorityFilter?: Set<string>;
}

export const SIGNAL_CONFIG: Record<string, SignalStyleConfig> = {
  security_alert: { icon: '🛡', color: 'text-red-400', borderColor: 'border-red-500/30', bgColor: 'bg-red-500/10' },
  breaking_change: { icon: '⚠', color: 'text-amber-400', borderColor: 'border-amber-500/30', bgColor: 'bg-amber-500/10' },
  tool_discovery: { icon: '🔧', color: 'text-blue-400', borderColor: 'border-blue-500/30', bgColor: 'bg-blue-500/10' },
  tech_trend: { icon: '📈', color: 'text-purple-400', borderColor: 'border-purple-500/30', bgColor: 'bg-purple-500/10' },
  learning: { icon: '📚', color: 'text-green-400', borderColor: 'border-green-500/30', bgColor: 'bg-green-500/10' },
  competitive_intel: { icon: '🏢', color: 'text-cyan-400', borderColor: 'border-cyan-500/30', bgColor: 'bg-cyan-500/10' },
};

export const PRIORITY_CONFIG: Record<string, PriorityStyleConfig> = {
  critical: { label: 'CRITICAL', color: 'text-red-400', bgColor: 'bg-red-500/20', dot: 'bg-red-400' },
  alert: { label: 'ALERT', color: 'text-orange-400', bgColor: 'bg-orange-500/20', dot: 'bg-orange-400' },
  advisory: { label: 'ADVISORY', color: 'text-yellow-400', bgColor: 'bg-yellow-500/20', dot: 'bg-yellow-400' },
  watch: { label: 'WATCH', color: 'text-text-secondary', bgColor: 'bg-gray-500/20', dot: 'bg-gray-400' },
};

export const SIGNAL_LABELS: Record<string, string> = {
  security_alert: 'Security',
  breaking_change: 'Breaking',
  tool_discovery: 'Tools',
  tech_trend: 'Trends',
  learning: 'Learning',
  competitive_intel: 'Competitive',
};

export const LANES: LaneConfig[] = [
  {
    key: 'critical',
    label: 'Critical Now',
    icon: '🔴',
    color: 'text-red-400',
    borderColor: 'border-red-500/20',
    types: new Set(['security_alert', 'breaking_change']),
    priorityFilter: new Set(['critical', 'alert']),
  },
  {
    key: 'stack',
    label: 'Stack Updates',
    icon: '📦',
    color: 'text-amber-400',
    borderColor: 'border-amber-500/20',
    types: new Set(['security_alert', 'breaking_change', 'tool_discovery']),
  },
  {
    key: 'learning',
    label: 'Learning & Trends',
    icon: '📈',
    color: 'text-blue-400',
    borderColor: 'border-blue-500/20',
    types: new Set(['learning', 'tech_trend', 'competitive_intel']),
  },
];
