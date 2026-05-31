// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { getRelevancePresentation } from '../../utils/score';
import { isAbstentionSynthesis, parseAbstention } from './briefing-synthesis-helpers';
import type { MorningBriefData, SynthesisCluster } from '../../store/slice-types';

interface MorningBriefPanelProps {
  morningBriefData: MorningBriefData;
  morningBriefSynthesis: string | null;
  morningBriefClusters: SynthesisCluster[] | null;
}

export const MorningBriefPanel = memo(function MorningBriefPanel({
  morningBriefData,
  morningBriefSynthesis,
  morningBriefClusters,
}: MorningBriefPanelProps) {
  const { t } = useTranslation();

  return (
    <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg space-y-4">
      <div className="bg-bg-secondary rounded-lg border border-border">
        <div className="px-5 pt-5 pb-3 border-b border-border flex items-center justify-between gap-3">
          <h2 className="text-[9px] font-semibold tracking-[0.12em] text-text-muted uppercase">
            {t('briefing.intelligenceBriefing')}
          </h2>
          <div className="flex items-center gap-2 text-[10px] text-text-muted">
            <span className="inline-block w-1.5 h-1.5 rounded-full bg-[#D4AF37] animate-pulse" />
            <span>{t('briefing.analysisRunning', 'Analysis running...')}</span>
          </div>
        </div>
        <div className="p-5 space-y-4">
          {morningBriefData.dataFreshness?.is_stale ? (
            <div className="flex items-start gap-2 px-3 py-2 rounded bg-[#EF4444]/10 border border-[#EF4444]/30">
              <span className="inline-block w-2 h-2 rounded-full bg-error mt-0.5 flex-shrink-0" />
              <div>
                <p className="text-xs text-error">
                  {t('briefing.staleData', 'Sources offline')}
                  {morningBriefData.dataFreshness.newest_source_check_age_hours != null && (
                    <span className="text-error/70">
                      {' — '}{t('briefing.lastFetch', 'last fetch {{hours}}h ago', { hours: Math.round(morningBriefData.dataFreshness.newest_source_check_age_hours) })}
                    </span>
                  )}
                </p>
                <p className="text-[10px] text-error/60 mt-0.5">
                  {t('briefing.staleHint', 'Check Settings → Sources or verify your internet connection')}
                </p>
              </div>
            </div>
          ) : morningBriefData.dataFreshness?.no_recent_fetches ? (
            <div className="flex items-start gap-2 px-3 py-2 rounded bg-[#D4AF37]/10 border border-[#D4AF37]/30">
              <span className="inline-block w-2 h-2 rounded-full bg-[#D4AF37] mt-0.5 flex-shrink-0" />
              <p className="text-xs text-[#D4AF37]">
                {t('briefing.noRecentFetches', 'No source checks in 24h — showing last known intelligence')}
              </p>
            </div>
          ) : null}
          {isAbstentionSynthesis(morningBriefSynthesis) ? (
            <div className="py-6 text-center space-y-2">
              <p className="text-xs text-text-muted italic">
                {parseAbstention(morningBriefSynthesis ?? '').headline}
              </p>
            </div>
          ) : morningBriefClusters && morningBriefClusters.length > 0 ? (
            <div className="pb-3 mb-1 border-b border-border">
              <h3 className="text-[9px] font-semibold tracking-[0.1em] text-[#D4AF37] uppercase mb-2">
                {t('briefing.synthesis', 'Synthesis')}
              </h3>
              <div className="space-y-2.5">
                {morningBriefClusters.map((cluster, i) => (
                  <div key={i} className="bg-bg-tertiary rounded-md px-3 py-2.5 space-y-1.5">
                    <div className="flex items-center gap-2">
                      <span className={`text-[8px] font-mono px-1.5 py-0.5 rounded ${
                        cluster.confidence >= 0.8 ? 'bg-success/15 text-success' :
                        cluster.confidence >= 0.5 ? 'bg-[#D4AF37]/15 text-[#D4AF37]' :
                        'bg-text-muted/15 text-text-muted'
                      }`}>
                        {Math.round(cluster.confidence * 100)}%
                      </span>
                      <p className="text-xs text-text-primary leading-snug">{cluster.insight}</p>
                    </div>
                    <p className="text-[10px] text-text-muted leading-relaxed pl-[38px]">{cluster.action}</p>
                  </div>
                ))}
              </div>
            </div>
          ) : morningBriefSynthesis ? (
            <div className="pb-3 mb-1 border-b border-border">
              <h3 className="text-[9px] font-semibold tracking-[0.1em] text-[#D4AF37] uppercase mb-2">
                {t('briefing.synthesis', 'Synthesis')}
              </h3>
              {(() => {
                const provenanceMatch = morningBriefSynthesis?.match(/^([\s\S]*?)\n\n(\(\d+ signals across .+\))$/);
                if (provenanceMatch) {
                  return (
                    <>
                      <p className="text-xs text-text-secondary leading-relaxed whitespace-pre-wrap">{provenanceMatch[1]}</p>
                      <p className="text-[9px] font-mono text-text-muted/60 mt-1.5">{provenanceMatch[2]}</p>
                    </>
                  );
                }
                return <p className="text-xs text-text-secondary leading-relaxed whitespace-pre-wrap">{morningBriefSynthesis}</p>;
              })()}
            </div>
          ) : null}
          {morningBriefData.items.length > 0 && (
          <div>
            <h3 className="text-[9px] font-semibold tracking-[0.1em] text-text-muted uppercase mb-2">
              {t('briefing.sourceItems', 'Source items')}
            </h3>
            <div className="space-y-2">
              {morningBriefData.items.slice(0, 8).map((item) => (
                <div
                  key={`${item.sourceType}:${item.title}`}
                  className="block pl-2 border-l-2 border-border py-1"
                >
                  <p className="text-xs text-text-primary leading-snug line-clamp-2">{item.title}</p>
                  <div className="flex items-center gap-2 mt-1">
                    <span className="text-[9px] font-mono text-text-muted uppercase tracking-wider">
                      {item.sourceType}
                    </span>
                    <span className={`text-[9px] font-medium uppercase tracking-wider ${getRelevancePresentation(item.score).colorClass}`}>
                      {t(getRelevancePresentation(item.score).labelKey)}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </div>
          )}
        </div>
      </div>
    </section>
  );
});
