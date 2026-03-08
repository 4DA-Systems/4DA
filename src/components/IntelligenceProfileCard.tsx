import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store';
import { useLicense } from '../hooks/use-license';

export function IntelligenceProfileCard() {
  const learnedAffinities = useAppStore(s => s.learnedAffinities) ?? [];
  const pulse = useAppStore(s => s.intelligencePulse);

  if (learnedAffinities.length === 0 && (!pulse || pulse.total_cycles === 0)) {
    return null;
  }

  const positiveAffinities = learnedAffinities.filter(a => a.affinity_score > 0);
  const topByStrength = [...learnedAffinities]
    .sort((a, b) => Math.abs(b.affinity_score) - Math.abs(a.affinity_score))
    .slice(0, 3);
  const displayAffinities = positiveAffinities.length > 0 ? positiveAffinities.slice(0, 3) : topByStrength;

  const accuracy = pulse?.calibration_accuracy ?? 0;
  const accuracyPct = Math.round(accuracy * 100);
  const accuracyColor = accuracyPct >= 70 ? 'text-green-400' : accuracyPct >= 40 ? 'text-amber-400' : 'text-red-400';

  return (
    <div className="space-y-3">
      {/* Autophagy Accuracy Card */}
      {pulse && pulse.total_cycles > 0 && (
        <div className="bg-[#1F1F1F] rounded-lg border border-border p-4 flex items-center gap-4">
          <div className="flex-shrink-0 w-12 h-12 rounded-lg bg-bg-tertiary flex items-center justify-center">
            <span className={`text-lg font-bold ${accuracyColor}`}>{accuracyPct}%</span>
          </div>
          <div className="flex-1 min-w-0">
            <h3 className="text-xs font-medium text-white">Autophagy Accuracy</h3>
            <p className="text-[10px] text-text-muted mt-0.5">
              {pulse.total_cycles} learning cycles &middot; {learnedAffinities.length} topics &middot; {pulse.items_analyzed_7d.toLocaleString()} items (7d)
            </p>
          </div>
          <div className="flex-shrink-0 w-24 h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div className={`h-full rounded-full transition-all ${accuracyPct >= 70 ? 'bg-green-500' : accuracyPct >= 40 ? 'bg-amber-500' : 'bg-red-500'}`}
              style={{ width: `${accuracyPct}%` }} />
          </div>
        </div>
      )}

      {/* Knowledge Gaps Card */}
      <KnowledgeGapsCard />

      {/* Intelligence Profile */}
      <div className="bg-[#1F1F1F] rounded-lg border border-border p-5">
        <h3 className="text-sm font-medium text-white mb-3">Your Intelligence Profile</h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {/* Top Affinities */}
          <div>
            <span className="text-[10px] text-text-muted uppercase tracking-wider">
              {positiveAffinities.length > 0 ? 'Top Affinities' : 'Strongest Signals'}
            </span>
            {displayAffinities.length > 0 ? (
              <div className="mt-1.5 space-y-1">
                {displayAffinities.map(a => (
                  <div key={a.topic} className="flex items-center gap-2">
                    <span className="text-xs text-white truncate flex-1">{a.topic}</span>
                    <div className="w-12 h-1 bg-bg-tertiary rounded-full overflow-hidden flex-shrink-0">
                      <div
                        className={`h-full rounded-full ${a.affinity_score > 0 ? 'bg-[#D4AF37]' : 'bg-[#666666]'}`}
                        style={{ width: `${Math.min(Math.abs(a.affinity_score) * 100, 100)}%` }}
                      />
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-[10px] text-text-muted mt-1.5">Interact with results to build affinities</p>
            )}
          </div>
          {/* Learning Velocity */}
          <div>
            <span className="text-[10px] text-text-muted uppercase tracking-wider">Learning Velocity</span>
            <p className="text-lg font-semibold text-white mt-1">
              {learnedAffinities.length}
              <span className="text-xs font-normal text-text-muted ml-1">topics learned</span>
            </p>
          </div>
          {/* System Activity */}
          <div>
            <span className="text-[10px] text-text-muted uppercase tracking-wider">System Activity</span>
            {pulse ? (
              <div className="mt-1.5 space-y-1">
                <p className="text-xs text-white">{pulse.items_analyzed_7d.toLocaleString()} items analyzed (7d)</p>
                <p className="text-xs text-text-secondary">
                  {pulse.items_surfaced_7d > 0
                    ? `${pulse.items_surfaced_7d} marked relevant`
                    : pulse.items_analyzed_7d > 0
                      ? 'Analyzing your preferences'
                      : '0 marked relevant'}
                </p>
                <p className="text-xs text-text-muted">{pulse.total_cycles} learning cycles complete</p>
              </div>
            ) : (
              <p className="text-[10px] text-text-muted mt-1.5">Analysis data will appear after first cycle</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Knowledge Gaps Card (compact, in briefing)
// ============================================================================

interface KnowledgeGap {
  dependency: string;
  gap_type: string;
  gap_message: string;
  severity: string;
  days_since_content: number | null;
}

function KnowledgeGapsCard() {
  const { isPro } = useLicense();
  const [gaps, setGaps] = useState<KnowledgeGap[]>([]);

  useEffect(() => {
    if (!isPro) return;
    invoke<KnowledgeGap[]>('get_knowledge_gaps')
      .then(g => setGaps(g))
      .catch(() => {});
  }, [isPro]);

  if (gaps.length === 0) return null;

  return (
    <div className="bg-[#1F1F1F] rounded-lg border border-amber-500/20 p-4">
      <h3 className="text-xs font-medium text-amber-400 mb-2">Knowledge Gaps ({gaps.length})</h3>
      <div className="flex flex-wrap gap-1.5">
        {gaps.slice(0, 8).map(gap => (
          <span key={gap.dependency} className="px-2 py-0.5 text-[10px] bg-amber-500/10 text-amber-300 rounded-full border border-amber-500/15">
            {gap.dependency}
            {gap.days_since_content != null && <span className="text-amber-500/60 ml-1">({gap.days_since_content}d)</span>}
          </span>
        ))}
        {gaps.length > 8 && (
          <span className="text-[10px] text-gray-500 self-center">+{gaps.length - 8} more</span>
        )}
      </div>
    </div>
  );
}
