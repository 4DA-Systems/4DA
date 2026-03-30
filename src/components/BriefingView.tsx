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
    freeBriefing, freeBriefingLoading,
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
          <div className="bg-bg-secondary rounded-lg border border-border p-5">
            <h2 className="font-medium text-white mb-3">{t('briefing.dailyOverview')}</h2>
            <div className="space-y-3">
              {freeBriefing.top_items?.map((item, i) => (
                <div key={i} className="flex items-start gap-3">
                  <span className="text-xs text-orange-400 font-mono font-medium flex-shrink-0 mt-0.5">{item.score}</span>
                  <div className="min-w-0">
                    {item.url ? (
                      <button
                        onClick={() => import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl(item.url!)).catch(() => window.open(item.url!, '_blank', 'noopener,noreferrer'))}
                        className="text-sm text-white hover:text-orange-400 text-start transition-colors"
                      >
                        {getTranslated(`free_${i}`, item.title)}
                      </button>
                    ) : (
                      <span className="text-sm text-white">{getTranslated(`free_${i}`, item.title)}</span>
                    )}
                    <span className="text-xs text-text-muted ms-2">{item.source}</span>
                  </div>
                </div>
              ))}
            </div>
            {freeBriefing.stack_alerts && freeBriefing.stack_alerts.length > 0 && (
              <div className="mt-4 pt-3 border-t border-border">
                <h3 className="text-xs font-medium text-amber-400 mb-2">{t('briefing.stackAlerts')}</h3>
                {freeBriefing.stack_alerts.map((alert, i) => (
                  <div key={i} className="text-xs text-text-secondary py-0.5">{getTranslated(`alert_${i}`, alert.title)}</div>
                ))}
              </div>
            )}
            <div className="mt-3 pt-3 border-t border-border flex items-center justify-between">
              <span className="text-xs text-text-muted">{t('briefing.itemsAnalyzed', { count: freeBriefing.total_items })}</span>
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
