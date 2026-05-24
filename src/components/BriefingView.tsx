// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useState, useEffect, memo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { getRelevancePresentation } from '../utils/score';
import { BriefingSkeleton } from './briefing/BriefingSkeleton';
import { BriefingContentPanel } from './briefing/BriefingContentPanel';
import { InstantSnapshotPanel } from './briefing/InstantSnapshotPanel';
import { PersonalizeNudge } from './briefing/PersonalizeNudge';
import { BriefingLoadingState, BriefingReadyState } from './BriefingEmptyStates';
import { BriefingWarmupState } from './BriefingWarmupState';
import { EngagementPulse } from './EngagementPulse';
import { useLicense } from '../hooks/use-license';
import { useBriefingDerived } from '../hooks/use-briefing-derived';
import { useTranslatedContent } from './ContentTranslationProvider';
import { isAbstentionSynthesis, parseAbstention } from './briefing/briefing-synthesis-helpers';
import type { SourceRelevance } from '../types';

export const BriefingView = memo(function BriefingView() {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();

  const {
    briefing, results, isLoading, analysisComplete, feedbackGiven,
    lastBackgroundResultsAt, sourceHealth,
    freeBriefing, freeBriefingLoading, morningBriefSynthesis, morningBriefClusters, morningBriefData, instantSnapshot,
  } = useAppStore(
    useShallow((s) => ({
      briefing: s.aiBriefing,
      results: s.appState.relevanceResults,
      isLoading: s.appState.loading,
      analysisComplete: s.appState.analysisComplete,
      feedbackGiven: s.feedbackGiven,
      lastBackgroundResultsAt: s.lastBackgroundResultsAt,
      sourceHealth: s.sourceHealth,
      freeBriefing: s.freeBriefing,
      freeBriefingLoading: s.freeBriefingLoading,
      morningBriefSynthesis: s.morningBriefSynthesis,
      morningBriefClusters: s.morningBriefClusters,
      morningBriefData: s.morningBriefData,
      instantSnapshot: s.instantSnapshot,
    })),
  );

  const generateBriefing = useAppStore(s => s.generateBriefing);
  const recordInteraction = useAppStore(s => s.recordInteraction);
  const setActiveView = useAppStore(s => s.setActiveView);
  const addToast = useAppStore(s => s.addToast);
  const generateFreeBriefing = useAppStore(s => s.generateFreeBriefing);
  const startAnalysis = useAppStore(s => s.startAnalysis);
  const setShowSettings = useAppStore(s => s.setShowSettings);

  // First-run personalization nudge
  const isFirstRun = useAppStore(s => s.isFirstRun);
  const userContext = useAppStore(s => s.userContext);
  const [personalizeCardDismissed, setPersonalizeCardDismissed] = useState(false);
  const showPersonalizeNudge = isFirstRun
    && !personalizeCardDismissed
    && (!userContext?.interests || userContext.interests.length === 0);

  const { isPro } = useLicense();

  const handleSave = useCallback((it: SourceRelevance) => { void recordInteraction(it.id, 'save', it); }, [recordInteraction]);
  const handleDismiss = useCallback((it: SourceRelevance) => { void recordInteraction(it.id, 'dismiss', it); }, [recordInteraction]);
  const handleRecordClick = useCallback((it: SourceRelevance) => { void recordInteraction(it.id, 'click', it); }, [recordInteraction]);

  // Listen for standing query matches
  useEffect(() => {
    const unlisten = listen<Array<{ query_id: number; query_text: string; new_matches: number; example_title: string | null }>>(
      'standing-query-matches',
      (event) => {
        const alerts = event.payload.filter(a => a.new_matches > 0);
        if (alerts.length > 0) {
          const msg = alerts.length === 1
            ? t('standingQueries.singleMatch', { query: alerts[0]!.query_text, count: alerts[0]!.new_matches })
            : t('standingQueries.multiMatch', { count: alerts.length });
          addToast('info', msg);
        }
      },
    );
    return () => { void unlisten.then(fn => fn()); };
  }, [addToast, t]);

  // Auto-generate free briefing when analysis completes
  useEffect(() => {
    if (analysisComplete && results.length > 0 && !freeBriefing && !freeBriefingLoading) {
      void generateFreeBriefing();
    }
  }, [analysisComplete, results.length, freeBriefing, freeBriefingLoading, generateFreeBriefing]);

  const { signalItems, topItems } =
    useBriefingDerived(results, sourceHealth, briefing, lastBackgroundResultsAt);

  // Loading skeleton
  if (briefing.loading) {
    return <BriefingSkeleton />;
  }

  // Sovereign Cold Boot — instant first paint of yesterday's briefing.
  // Naturally superseded by the render waterfall once aiBriefing.content
  // or analysisComplete populates.
  if (!briefing.content && !analysisComplete && instantSnapshot) {
    return <InstantSnapshotPanel snapshot={instantSnapshot} />;
  }

  // Empty state: no briefing content and not generating
  if (!briefing.content) {
    if (isLoading) return <BriefingLoadingState />;

    // Free briefing for non-Pro users
    if (!isPro && freeBriefing && !freeBriefing.empty) {
      return (
        <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg space-y-4">
          {showPersonalizeNudge && (
            <PersonalizeNudge
              onOpenSettings={() => setShowSettings(true)}
              onDismiss={() => setPersonalizeCardDismissed(true)}
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
                onClick={() => { void generateBriefing(); }}
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
    }

    // Morning briefing items — fills the gap between startup and analysis completion.
    // The T+3s morning check produces scored items from the DB; render them while
    // the full analysis runs in the background.
    // Also render when data is stale (0 items but staleness flag set) so the user
    // sees the problem instead of silence masquerading as "all clear."
    if (morningBriefData && (morningBriefData.items.length > 0 || morningBriefData.dataFreshness?.is_stale)) {
      return (
        <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg space-y-4">
          <div className="bg-bg-secondary rounded-lg border border-border">
            <div className="px-5 pt-5 pb-3 border-b border-border flex items-center justify-between gap-3">
              <h2 className="text-[9px] font-semibold tracking-[0.12em] text-text-muted uppercase">
                {t('briefing.intelligenceBriefing')}
              </h2>
              <div className="flex items-center gap-2 text-[10px] text-text-muted">
                <span className="inline-block w-1.5 h-1.5 rounded-full bg-[#D4AF37] animate-pulse" />
                <span>{t('briefing.analysisRunning', 'Analysis running…')}</span>
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
                  {morningBriefData.items.map((item, i) => (
                    <div
                      key={i}
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
    }

    if (analysisComplete && results.length > 0) return <BriefingReadyState />;
    return <BriefingWarmupState onAnalyze={() => { void startAnalysis(); }} />;
  }

  // Main view: Intelligence Hierarchy (3 zones)
  return (
    <section aria-label={t('briefing.intelligenceBriefing')} className="bg-bg-primary rounded-lg space-y-5">
      {showPersonalizeNudge && (
        <PersonalizeNudge
          onOpenSettings={() => setShowSettings(true)}
          onDismiss={() => setPersonalizeCardDismissed(true)}
        />
      )}

      <BriefingContentPanel
        briefing={briefing}
        results={results}
        feedbackGiven={feedbackGiven}
        sourceHealth={sourceHealth}
        signalItems={signalItems}
        topItems={topItems}
        onSave={handleSave}
        onDismiss={handleDismiss}
        onRecordClick={handleRecordClick}
        setActiveView={setActiveView}
      />

      {/* Error display */}
      {briefing.error && (
        <div role="alert" className="p-4 bg-red-900/20 border border-red-500/30 rounded-lg">
          <div className="flex flex-col items-center justify-center gap-3 text-center">
            <p className="text-text-secondary text-sm">{t('error.generic')}</p>
            <button
              onClick={() => { void generateBriefing(); }}
              className="px-3 py-1.5 text-xs bg-bg-tertiary hover:bg-white/10 rounded transition-colors text-text-secondary"
              aria-label="Retry generating briefing"
            >
              {t('action.retry')}
            </button>
          </div>
        </div>
      )}
    </section>
  );
});
