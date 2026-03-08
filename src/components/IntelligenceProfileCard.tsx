import { useState, useEffect, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store';
import { useLicense } from '../hooks/use-license';

export const IntelligenceProfileCard = memo(function IntelligenceProfileCard() {
  const { t } = useTranslation();
  const learnedAffinities = useAppStore(s => s.learnedAffinities) ?? [];
  const pulse = useAppStore(s => s.intelligencePulse);

  const positiveAffinities = useMemo(() =>
    learnedAffinities.filter(a => a.affinity_score > 0),
  [learnedAffinities]);

  const topByStrength = useMemo(() =>
    [...learnedAffinities]
      .sort((a, b) => Math.abs(b.affinity_score) - Math.abs(a.affinity_score))
      .slice(0, 3),
  [learnedAffinities]);

  const displayAffinities = useMemo(() =>
    positiveAffinities.length > 0 ? positiveAffinities.slice(0, 3) : topByStrength,
  [positiveAffinities, topByStrength]);

  if (learnedAffinities.length === 0 && (!pulse || pulse.total_cycles === 0)) {
    return null;
  }

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
            <h3 className="text-xs font-medium text-white">{t('briefing.profile.autophagyAccuracy')}</h3>
            <p className="text-[10px] text-text-muted mt-0.5">
              {t('briefing.profile.autophagySummary', { cycles: pulse.total_cycles, topics: learnedAffinities.length, items: pulse.items_analyzed_7d.toLocaleString() })}
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
        <h3 className="text-sm font-medium text-white mb-3">{t('briefing.profile.title')}</h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {/* Top Affinities */}
          <div>
            <span className="text-[10px] text-text-muted uppercase tracking-wider">
              {positiveAffinities.length > 0 ? t('briefing.profile.topAffinities') : t('briefing.profile.strongestSignals')}
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
              <p className="text-[10px] text-text-muted mt-1.5">{t('briefing.profile.interactHint')}</p>
            )}
          </div>
          {/* Learning Velocity */}
          <div>
            <span className="text-[10px] text-text-muted uppercase tracking-wider">{t('briefing.profile.learningVelocity')}</span>
            <p className="text-lg font-semibold text-white mt-1">
              {learnedAffinities.length}
              <span className="text-xs font-normal text-text-muted ml-1">{t('briefing.profile.topicsLearned')}</span>
            </p>
          </div>
          {/* System Activity */}
          <div>
            <span className="text-[10px] text-text-muted uppercase tracking-wider">{t('briefing.profile.systemActivity')}</span>
            {pulse ? (
              <div className="mt-1.5 space-y-1">
                <p className="text-xs text-white">{t('briefing.profile.itemsAnalyzed7d', { items: pulse.items_analyzed_7d.toLocaleString() })}</p>
                <p className="text-xs text-text-secondary">
                  {pulse.items_surfaced_7d > 0
                    ? t('briefing.profile.markedRelevant', { count: pulse.items_surfaced_7d })
                    : pulse.items_analyzed_7d > 0
                      ? t('briefing.profile.analyzingPreferences')
                      : t('briefing.profile.markedRelevant', { count: 0 })}
                </p>
                <p className="text-xs text-text-muted">{t('briefing.profile.cyclesComplete', { count: pulse.total_cycles })}</p>
              </div>
            ) : (
              <p className="text-[10px] text-text-muted mt-1.5">{t('briefing.profile.noDataYet')}</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
});

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
  const { t } = useTranslation();
  const { isPro } = useLicense();
  const [gaps, setGaps] = useState<KnowledgeGap[]>([]);

  useEffect(() => {
    if (!isPro) return;
    invoke<KnowledgeGap[]>('get_knowledge_gaps')
      .then(g => setGaps(g))
      .catch(() => {});
  }, [isPro]);

  if (!isPro) return null;
  if (gaps.length === 0) return null;

  return (
    <div className="bg-[#1F1F1F] rounded-lg border border-amber-500/20 p-4">
      <h3 className="text-xs font-medium text-amber-400 mb-2">{t('briefing.profile.knowledgeGaps', { count: gaps.length })}</h3>
      <div className="flex flex-wrap gap-1.5">
        {gaps.slice(0, 8).map(gap => (
          <span key={gap.dependency} className="px-2 py-0.5 text-[10px] bg-amber-500/10 text-amber-300 rounded-full border border-amber-500/15">
            {gap.dependency}
            {gap.days_since_content != null && <span className="text-amber-500/60 ml-1">({gap.days_since_content}d)</span>}
          </span>
        ))}
        {gaps.length > 8 && (
          <span className="text-[10px] text-gray-500 self-center">{t('briefing.profile.moreGaps', { count: gaps.length - 8 })}</span>
        )}
      </div>
    </div>
  );
}
