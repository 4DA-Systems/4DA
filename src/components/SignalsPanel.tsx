import { useState, useMemo } from 'react';
import type { SourceRelevance } from '../types';

// ============================================================================
// Types
// ============================================================================

interface SignalItem {
  id: number;
  title: string;
  url: string | null;
  top_score: number;
  source_type: string;
  signal_type: string;
  signal_priority: string;
  signal_action: string;
  signal_triggers: string[];
}

interface SignalsPanelProps {
  results: SourceRelevance[];
}

// ============================================================================
// Constants
// ============================================================================

const SIGNAL_CONFIG: Record<string, { icon: string; color: string; borderColor: string; bgColor: string }> = {
  security_alert: { icon: '🛡', color: 'text-red-400', borderColor: 'border-red-500/30', bgColor: 'bg-red-500/10' },
  breaking_change: { icon: '⚠', color: 'text-amber-400', borderColor: 'border-amber-500/30', bgColor: 'bg-amber-500/10' },
  tool_discovery: { icon: '🔧', color: 'text-blue-400', borderColor: 'border-blue-500/30', bgColor: 'bg-blue-500/10' },
  tech_trend: { icon: '📈', color: 'text-purple-400', borderColor: 'border-purple-500/30', bgColor: 'bg-purple-500/10' },
  learning: { icon: '📚', color: 'text-green-400', borderColor: 'border-green-500/30', bgColor: 'bg-green-500/10' },
  competitive_intel: { icon: '🏢', color: 'text-cyan-400', borderColor: 'border-cyan-500/30', bgColor: 'bg-cyan-500/10' },
};

const PRIORITY_CONFIG: Record<string, { label: string; color: string; bgColor: string; dot: string }> = {
  critical: { label: 'CRITICAL', color: 'text-red-400', bgColor: 'bg-red-500/20', dot: 'bg-red-400' },
  high: { label: 'HIGH', color: 'text-orange-400', bgColor: 'bg-orange-500/20', dot: 'bg-orange-400' },
  medium: { label: 'MED', color: 'text-yellow-400', bgColor: 'bg-yellow-500/20', dot: 'bg-yellow-400' },
  low: { label: 'LOW', color: 'text-gray-400', bgColor: 'bg-gray-500/20', dot: 'bg-gray-400' },
};

const SIGNAL_LABELS: Record<string, string> = {
  security_alert: 'Security',
  breaking_change: 'Breaking',
  tool_discovery: 'Tools',
  tech_trend: 'Trends',
  learning: 'Learning',
  competitive_intel: 'Competitive',
};

// ============================================================================
// Component
// ============================================================================

export const SignalsPanel = ({ results }: SignalsPanelProps) => {
  const [expanded, setExpanded] = useState(true);
  const [typeFilter, setTypeFilter] = useState<string | null>(null);
  const [priorityFilter, setPriorityFilter] = useState<string | null>(null);

  const { signals, filtered, typeCounts, priorityCounts } = useMemo(() => {
    const signals: SignalItem[] = results
      .filter((r) => r.signal_type && r.signal_priority && r.signal_action)
      .map((r) => ({
        id: r.id,
        title: r.title,
        url: r.url,
        top_score: r.top_score,
        source_type: r.source_type || 'unknown',
        signal_type: r.signal_type!,
        signal_priority: r.signal_priority!,
        signal_action: r.signal_action!,
        signal_triggers: r.signal_triggers || [],
      }));

    const priorityOrder: Record<string, number> = { critical: 4, high: 3, medium: 2, low: 1 };
    const sorted = [...signals].sort((a, b) => {
      const pd = (priorityOrder[b.signal_priority] || 0) - (priorityOrder[a.signal_priority] || 0);
      if (pd !== 0) return pd;
      return b.top_score - a.top_score;
    });

    const filtered = sorted
      .filter((s) => !typeFilter || s.signal_type === typeFilter)
      .filter((s) => !priorityFilter || s.signal_priority === priorityFilter);

    const typeCounts: Record<string, number> = {};
    const priorityCounts: Record<string, number> = {};
    for (const s of signals) {
      typeCounts[s.signal_type] = (typeCounts[s.signal_type] || 0) + 1;
      priorityCounts[s.signal_priority] = (priorityCounts[s.signal_priority] || 0) + 1;
    }

    return { signals, sorted, filtered, typeCounts, priorityCounts };
  }, [results, typeFilter, priorityFilter]);

  if (signals.length === 0) return null;

  const criticalCount = priorityCounts['critical'] || 0;
  const highCount = priorityCounts['high'] || 0;

  return (
    <div className="mb-6 bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
      {/* Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full px-5 py-4 border-b border-[#2A2A2A] flex items-center justify-between hover:bg-[#1A1A1A] transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center">
            <span className="text-gray-400">⚡</span>
          </div>
          <div className="text-left">
            <h2 className="font-medium text-white">Signals</h2>
            <p className="text-xs text-gray-500">
              {signals.length} actionable
              {criticalCount > 0 && (
                <span className="ml-2 text-red-400">{criticalCount} critical</span>
              )}
              {highCount > 0 && (
                <span className="ml-2 text-orange-400">{highCount} high</span>
              )}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          {/* Priority dots summary */}
          <div className="flex gap-1">
            {criticalCount > 0 && <span className="w-2 h-2 rounded-full bg-red-400" title={`${criticalCount} critical`} />}
            {highCount > 0 && <span className="w-2 h-2 rounded-full bg-orange-400" title={`${highCount} high`} />}
            {(priorityCounts['medium'] || 0) > 0 && <span className="w-2 h-2 rounded-full bg-yellow-400" title={`${priorityCounts['medium']} medium`} />}
          </div>
          <span className="text-gray-500 text-sm">{expanded ? '▾' : '▸'}</span>
        </div>
      </button>

      {expanded && (
        <div className="p-4">
          {/* Filters */}
          <div className="flex flex-wrap gap-2 mb-4">
            {/* Type filters */}
            {Object.entries(typeCounts).map(([type, count]) => {
              const config = SIGNAL_CONFIG[type];
              const isActive = typeFilter === type;
              return (
                <button
                  key={type}
                  onClick={() => setTypeFilter(isActive ? null : type)}
                  className={`px-2.5 py-1 text-[11px] rounded-lg border transition-all flex items-center gap-1.5 ${
                    isActive
                      ? `${config?.bgColor || 'bg-white/10'} ${config?.color || 'text-white'} ${config?.borderColor || 'border-white/20'}`
                      : 'bg-[#1F1F1F] text-gray-400 border-[#2A2A2A] hover:border-[#3A3A3A]'
                  }`}
                >
                  <span>{config?.icon || '?'}</span>
                  <span>{SIGNAL_LABELS[type] || type}</span>
                  <span className="text-[10px] opacity-60">{count}</span>
                </button>
              );
            })}

            {/* Divider */}
            {Object.keys(typeCounts).length > 0 && (
              <span className="self-center text-[#2A2A2A]">|</span>
            )}

            {/* Priority filters */}
            {['critical', 'high', 'medium', 'low'].map((p) => {
              const count = priorityCounts[p] || 0;
              if (count === 0) return null;
              const config = PRIORITY_CONFIG[p];
              const isActive = priorityFilter === p;
              return (
                <button
                  key={p}
                  onClick={() => setPriorityFilter(isActive ? null : p)}
                  className={`px-2 py-1 text-[10px] font-medium rounded-lg border transition-all flex items-center gap-1.5 ${
                    isActive
                      ? `${config.bgColor} ${config.color} border-current`
                      : 'bg-[#1F1F1F] text-gray-500 border-[#2A2A2A] hover:border-[#3A3A3A]'
                  }`}
                >
                  <span className={`w-1.5 h-1.5 rounded-full ${config.dot}`} />
                  <span>{config.label}</span>
                  <span className="opacity-60">{count}</span>
                </button>
              );
            })}

            {/* Clear filters */}
            {(typeFilter || priorityFilter) && (
              <button
                onClick={() => { setTypeFilter(null); setPriorityFilter(null); }}
                className="px-2 py-1 text-[10px] text-gray-500 hover:text-white transition-colors"
              >
                Clear
              </button>
            )}
          </div>

          {/* Signal Items */}
          <div className="space-y-2 max-h-[400px] overflow-y-auto">
            {filtered.length === 0 ? (
              <p className="text-center text-sm text-gray-500 py-4">No signals match filters</p>
            ) : (
              filtered.map((signal) => (
                <SignalRow key={signal.id} signal={signal} />
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
};

// ============================================================================
// Signal Row
// ============================================================================

const SignalRow = ({ signal }: { signal: SignalItem }) => {
  const [showTriggers, setShowTriggers] = useState(false);
  const config = SIGNAL_CONFIG[signal.signal_type] || SIGNAL_CONFIG['tech_trend'];
  const priority = PRIORITY_CONFIG[signal.signal_priority] || PRIORITY_CONFIG['low'];

  return (
    <div
      className={`px-4 py-3 rounded-lg border ${config.borderColor} ${config.bgColor} transition-all hover:brightness-125`}
    >
      <div className="flex items-start gap-3">
        {/* Icon + Priority */}
        <div className="flex flex-col items-center gap-1 pt-0.5">
          <span className="text-base">{config.icon}</span>
          <span className={`w-2 h-2 rounded-full ${priority.dot}`} title={priority.label} />
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          {/* Top row: action text */}
          <p className={`text-sm font-medium ${config.color} leading-snug`}>
            {signal.signal_action}
          </p>

          {/* Title + meta */}
          <div className="flex items-center gap-2 mt-1">
            {signal.url ? (
              <a
                href={signal.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs text-gray-400 hover:text-white truncate transition-colors"
                title={signal.title}
              >
                {signal.title}
              </a>
            ) : (
              <span className="text-xs text-gray-400 truncate">{signal.title}</span>
            )}
          </div>

          {/* Bottom row: type badge + priority + score + triggers */}
          <div className="flex items-center gap-2 mt-2">
            <span className={`px-1.5 py-0.5 text-[10px] rounded ${config.bgColor} ${config.color} border ${config.borderColor}`}>
              {SIGNAL_LABELS[signal.signal_type] || signal.signal_type}
            </span>
            <span className={`px-1.5 py-0.5 text-[10px] font-medium rounded ${priority.bgColor} ${priority.color}`}>
              {priority.label}
            </span>
            <span className="text-[10px] text-gray-500">
              {Math.round(signal.top_score * 100)}% match
            </span>
            <span className="text-[10px] text-gray-600">{signal.source_type}</span>
            {signal.signal_triggers.length > 0 && (
              <button
                onClick={() => setShowTriggers(!showTriggers)}
                className="text-[10px] text-gray-500 hover:text-gray-300 transition-colors ml-auto"
              >
                {showTriggers ? 'hide triggers' : `${signal.signal_triggers.length} triggers`}
              </button>
            )}
          </div>

          {/* Trigger keywords */}
          {showTriggers && (
            <div className="flex flex-wrap gap-1 mt-2">
              {signal.signal_triggers.map((t, i) => (
                <span key={i} className="px-1.5 py-0.5 text-[10px] bg-[#1F1F1F] text-gray-400 rounded border border-[#2A2A2A]">
                  {t}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
