import { useState, useEffect, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { KnowledgeGap } from '../types';

const SEVERITY_CONFIG: Record<string, { color: string; bg: string; border: string }> = {
  critical: { color: 'text-red-400', bg: 'bg-red-500/10', border: 'border-red-500/20' },
  high: { color: 'text-amber-400', bg: 'bg-amber-500/10', border: 'border-amber-500/20' },
  medium: { color: 'text-yellow-400', bg: 'bg-yellow-500/10', border: 'border-yellow-500/20' },
  low: { color: 'text-gray-400', bg: 'bg-gray-500/10', border: 'border-gray-500/20' },
};

export const KnowledgeGapsPanel = memo(function KnowledgeGapsPanel() {
  const [gaps, setGaps] = useState<KnowledgeGap[]>([]);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    const load = async () => {
      try {
        const g = await invoke<KnowledgeGap[]>('get_knowledge_gaps');
        setGaps(g);
      } catch {
        // Knowledge gaps are optional
      }
    };
    load();
  }, []);

  if (gaps.length === 0) return null;

  const criticalCount = gaps.filter(g => g.gap_severity === 'critical' || g.gap_severity === 'high').length;

  return (
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full px-5 py-4 flex items-center justify-between hover:bg-[#1A1A1A] transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-gray-400">📖</span>
          </div>
          <div className="text-left">
            <h2 className="font-medium text-white text-sm">Knowledge Gaps</h2>
            <p className="text-xs text-gray-500">
              {gaps.length} gap{gaps.length !== 1 ? 's' : ''}
              {criticalCount > 0 && <span className="text-amber-400 ml-1">{criticalCount} need attention</span>}
            </p>
          </div>
        </div>
        <span className="text-gray-500 text-sm">{expanded ? '▾' : '▸'}</span>
      </button>

      {expanded && (
        <div className="p-4 space-y-2 border-t border-border">
          {gaps.map((gap, i) => {
            const sev = SEVERITY_CONFIG[gap.gap_severity] || SEVERITY_CONFIG.low;
            return (
              <div key={i} className={`px-4 py-3 rounded-lg border ${sev.border} ${sev.bg}`}>
                <div className="flex items-center gap-2">
                  <span className={`text-sm font-medium ${sev.color}`}>{gap.dependency}</span>
                  {gap.version && <span className="text-[10px] text-gray-500">v{gap.version}</span>}
                  <span className={`ml-auto text-[10px] px-1.5 py-0.5 rounded ${sev.bg} ${sev.color} border ${sev.border}`}>
                    {gap.gap_severity}
                  </span>
                </div>
                <p className="text-xs text-gray-400 mt-1">
                  {gap.missed_items.length} relevant article{gap.missed_items.length !== 1 ? 's' : ''} you haven't engaged with
                </p>
                {gap.missed_items.length > 0 && (
                  <div className="mt-2 space-y-1">
                    {gap.missed_items.slice(0, 3).map((item) => (
                      <div key={item.item_id} className="text-[11px]">
                        {item.url ? (
                          <a href={item.url} target="_blank" rel="noopener noreferrer" className="text-gray-400 hover:text-white transition-colors">
                            {item.title}
                          </a>
                        ) : (
                          <span className="text-gray-400">{item.title}</span>
                        )}
                      </div>
                    ))}
                    {gap.missed_items.length > 3 && (
                      <span className="text-[10px] text-gray-600">+{gap.missed_items.length - 3} more</span>
                    )}
                  </div>
                )}
                <div className="text-[10px] text-gray-600 mt-1">
                  {gap.days_since_last_engagement >= 999
                    ? 'No engagement recorded'
                    : `${gap.days_since_last_engagement}d since last engagement`} · {gap.project_path}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
});
