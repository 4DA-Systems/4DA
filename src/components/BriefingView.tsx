import { useCallback, useState, useEffect, memo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { BriefingSkeleton } from './briefing/BriefingSkeleton';
import { BriefingContentPanel } from './briefing/BriefingContentPanel';
import { PersonalizeNudge } from './briefing/PersonalizeNudge';
import { BriefingLoadingState, BriefingReadyState } from './BriefingEmptyStates';
import { BriefingWarmupState } from './BriefingWarmupState';
import { EngagementPulse } from './EngagementPulse';
import { useLicense } from '../hooks/use-license';
import { useBriefingDerived } from '../hooks/use-briefing-derived';
import { useTranslatedContent } from './ContentTranslationProvider';
import type { SourceRelevance } from '../types';

export const BriefingView = memo(function BriefingView() {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();

  const {
    briefing, results, isLoading, analysisComplete, feedbackGiven,
    lastBackgroundResultsAt, sourceHealth,
    freeBriefing, freeBriefingLoading, morningBriefSynthesis,
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

  const handleSave = useCallback((it: SourceRelevance) => recordInteraction(it.id, 'save', it), [recordInteraction]);
  const handleDismiss = useCallback((it: SourceRelevance) => recordInteraction(it.id, 'dismiss', it), [recordInteraction]);
  const handleRecordClick = useCallback((it: SourceRelevance) => recordInteraction(it.id, 'click', it), [recordInteraction]);

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
    return () => { unlisten.then(fn => fn()); };
  }, [addToast, t]);

  // Auto-generate free briefing when analysis completes
  useEffect(() => {
    if (analysisComplete && results.length > 0 && !freeBriefing && !freeBriefingLoading) {
      generateFreeBriefing();
    }
  }, [analysisComplete, results.length, freeBriefing, freeBriefingLoading, generateFreeBriefing]);

  const { signalItems, topItems } =
    useBriefingDerived(results, sourceHealth, briefing, lastBackgroundResultsAt);

  // Loading skeleton
  if (briefing.loading) {
    return <BriefingSkeleton />;
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
              {freeBriefing.signal_priorities && Object.keys(freeBriefing.signal_priorities).length > 0 && (
                <div className="flex gap-3 mt-2">
                  {(freeBriefing.signal_priorities.critical ?? 0) > 0 && (
                    <span className="text-[10px] font-mono text-red-400">{freeBriefing.signal_priorities.critical} critical</span>
                  )}
                  {(freeBriefing.signal_priorities.alert ?? 0) > 0 && (
                    <span className="text-[10px] font-mono text-orange-400">{freeBriefing.signal_priorities.alert} alert</span>
                  )}
                  {(freeBriefing.signal_priorities.advisory ?? 0) > 0 && (
                    <span className="text-[10px] font-mono text-amber-400">{freeBriefing.signal_priorities.advisory} advisory</span>
                  )}
                </div>
              )}
            </div>
            <div className="p-5 space-y-4">
              {morningBriefSynthesis && (
                <div className="pb-3 mb-1 border-b border-border">
                  <p className="text-xs text-text-secondary leading-relaxed whitespace-pre-wrap">{morningBriefSynthesis}</p>
                </div>
              )}
              {freeBriefing.wisdom_signals && freeBriefing.wisdom_signals.length > 0 && (
                <div>
                  <h3 className="text-[9px] font-semibold tracking-[0.1em] text-[#D4AF37] uppercase mb-2">Wisdom</h3>
                  <div className="space-y-2">
                    {freeBriefing.wisdom_signals.map((signal, i) => (
                      <div key={i} className="pl-2 border-l-2 border-[#D4AF37]/25 py-1">
                        <div className="flex items-center gap-2 mb-0.5">
                          <span className="text-[8px] font-mono font-bold tracking-wider text-[#D4AF37]/80 bg-[#D4AF37]/10 px-1.5 rounded">
                            {signal.signal_type === 'anti-pattern' ? 'AVOID' : 'VALIDATED'}
                          </span>
                          <span className="text-[10px] font-mono text-text-muted">{Math.round(signal.confidence * 100)}%</span>
                        </div>
                        <p className="text-xs text-text-secondary italic leading-relaxed">{signal.text}</p>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              <div>
                <h3 className="text-[9px] font-semibold tracking-[0.1em] text-text-muted uppercase mb-2">Signals</h3>
                <div className="space-y-1">
                  {freeBriefing.top_items?.map((item, i) => {
                    const pc = item.signal_priority === 'critical' ? 'bg-red-500'
                      : item.signal_priority === 'alert' ? 'bg-orange-500'
                      : item.signal_priority === 'advisory' ? 'bg-amber-500' : 'bg-blue-500';
                    return (
                      <div key={i} className="flex items-start gap-2.5 py-1.5 px-2 rounded hover:bg-white/[0.02] transition-colors">
                        <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 mt-1.5 ${pc}`} />
                        <div className="min-w-0 flex-1">
                          {item.url ? (
                            <button
                              onClick={() => import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl(item.url!)).catch(() => window.open(item.url!, '_blank', 'noopener,noreferrer'))}
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
                  <h3 className="text-[9px] font-semibold tracking-[0.1em] text-amber-400 uppercase mb-2">Blind Spots</h3>
                  <div className="space-y-1">
                    {freeBriefing.knowledge_gaps.map((gap, i) => (
                      <div key={i} className="flex items-center justify-between px-2 py-1 rounded bg-amber-500/[0.03]">
                        <span className="text-[11px] font-medium text-text-secondary">{gap.topic}</span>
                        <span className="text-[10px] font-mono text-text-muted">{gap.days_since_last}d</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
            <div className="px-5 py-3 border-t border-border flex items-center justify-between">
              <span className="text-[10px] font-mono text-text-muted">{freeBriefing.total_items} signals analyzed</span>
              <button
                onClick={generateBriefing}
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

    if (analysisComplete && results.length > 0) return <BriefingReadyState />;
    return <BriefingWarmupState onAnalyze={startAnalysis} />;
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
              onClick={generateBriefing}
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
