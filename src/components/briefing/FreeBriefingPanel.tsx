// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { useTranslatedContent } from '../ContentTranslationProvider';
import { EngagementPulse } from '../EngagementPulse';
import { PersonalizeNudge } from './PersonalizeNudge';
import { isAbstentionSynthesis, parseAbstention } from './briefing-synthesis-helpers';
import type { FreeBriefingData, SynthesisCluster } from '../../store/slice-types';

interface FreeBriefingPanelProps {
  freeBriefing: FreeBriefingData;
  morningBriefSynthesis: string | null;
  morningBriefClusters: SynthesisCluster[] | null;
  showPersonalizeNudge: boolean;
  onOpenSettings: () => void;
  onDismissPersonalize: () => void;
  onGenerateBriefing: () => void;
}

export const FreeBriefingPanel = memo(function FreeBriefingPanel({
  freeBriefing,
  morningBriefSynthesis,
  morningBriefClusters,
  showPersonalizeNudge,
  onOpenSettings,
  onDismissPersonalize,
  onGenerateBriefing,
}: FreeBriefingPanelProps) {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();

  return (
    <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg space-y-4">
      {showPersonalizeNudge && (
        <PersonalizeNudge
          onOpenSettings={onOpenSettings}
          onDismiss={onDismissPersonalize}
        />
      )}
      <div className="bg-bg-secondary rounded-lg border border-border">
        <div className="px-5 pt-5 pb-3 border-b border-border">
          <h2 className="text-[9px] font-semibold tracking-[0.12em] text-text-muted uppercase">{t('briefing.intelligenceBriefing')}</h2>
        </div>
        <div className="p-5 space-y-4">
          {/* Synthesis — structured clusters when available, prose fallback */}
          {isAbstentionSynthesis(morningBriefSynthesis) ? (
            <div className="py-6 text-center space-y-2">
              <p className="text-xs text-text-muted italic">
                {parseAbstention(morningBriefSynthesis ?? '').headline}
              </p>
              {parseAbstention(morningBriefSynthesis ?? '').telemetry != null && (
                <p className="text-[9px] font-mono text-text-muted/60">
                  {parseAbstention(morningBriefSynthesis ?? '').telemetry}
                </p>
              )}
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
              {(() => {
                const provenanceMatch = morningBriefSynthesis?.match(/(\(\d+ signals across .+\))$/);
                if (provenanceMatch) {
                  return <p className="text-[9px] font-mono text-text-muted/60 mt-2">{provenanceMatch[1]}</p>;
                }
                return null;
              })()}
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
          <div>
            <h3 className="text-[9px] font-semibold tracking-[0.1em] text-text-muted uppercase mb-2">{t('briefing.sectionSignals')}</h3>
            <div className="space-y-1">
              {freeBriefing.top_items?.map((item, i) => {
                const pc = 'bg-text-muted';
                return (
                  <div key={i} className="flex items-start gap-2.5 py-1.5 px-2 rounded hover:bg-white/[0.02] transition-colors">
                    <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 mt-1.5 ${pc}`} />
                    <div className="min-w-0 flex-1">
                      {item.url ? (
                        <button
                          onClick={() => { void import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl(item.url!)).catch(() => window.open(item.url!, '_blank', 'noopener,noreferrer')); }}
                          aria-label={`${t('feedback.openLink')}: ${item.title}`}
                          className="text-xs text-white hover:text-text-secondary text-start transition-colors leading-snug"
                        >
                          {getTranslated(`free_${i}`, item.title)}
                        </button>
                      ) : (
                        <span className="text-xs text-white leading-snug">{getTranslated(`free_${i}`, item.title)}</span>
                      )}
                      <div className="flex items-center gap-2 mt-0.5">
                        <span className="text-[9px] font-mono text-text-muted">{item.source}</span>
                        <span className="text-[9px] font-mono text-[#D4AF37]">{item.score}</span>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
          {freeBriefing.stack_alerts && freeBriefing.stack_alerts.length > 0 && (
            <div>
              <h3 className="text-[9px] font-semibold tracking-[0.1em] text-amber-400 uppercase mb-2">{t('briefing.stackAlerts')}</h3>
              {freeBriefing.stack_alerts.map((alert, i) => (
                <div key={i} className="text-xs text-text-secondary py-0.5 pl-2">{getTranslated(`alert_${i}`, alert.title)}</div>
              ))}
            </div>
          )}
          {freeBriefing.knowledge_gaps && freeBriefing.knowledge_gaps.length > 0 && (
            <div>
              <h3 className="text-[9px] font-semibold tracking-[0.1em] text-amber-400 uppercase mb-2">{t('briefing.sectionBlindSpots')}</h3>
              <div className="space-y-1">
                {freeBriefing.knowledge_gaps.map((gap, i) => (
                  <div key={i} className="flex items-center justify-between px-2 py-1 rounded bg-amber-500/[0.03]">
                    <span className="text-[11px] font-medium text-text-secondary">{gap.topic}</span>
                    <span className="text-[10px] font-mono text-text-muted">{t('briefing.daysSilent', { days: gap.days_since_last })}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
        <div className="px-5 py-3 border-t border-border flex items-center justify-between">
          <span className="text-[10px] font-mono text-text-muted">{t('briefing.signalsAnalyzed', { count: freeBriefing.total_items })}</span>
          <button
            onClick={() => { void onGenerateBriefing(); }}
            aria-label={t('briefing.generateAI')}
            className="px-3 py-1.5 text-xs bg-orange-500/10 text-orange-400 border border-orange-500/20 rounded-lg hover:bg-orange-500/20 transition-all font-medium"
          >
            {t('briefing.generateAI')}
          </button>
        </div>
      </div>
      <EngagementPulse />
    </section>
  );
});
