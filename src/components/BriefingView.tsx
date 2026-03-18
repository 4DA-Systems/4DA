import { useCallback, useState, useEffect, memo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { BriefingCard } from './BriefingCard';
import { SignalActionCard } from './briefing/SignalActionCard';
import { BriefingAtmosphere, RelativeTimestamp, SKELETON_WIDTHS } from './briefing/BriefingHelpers';
import { BriefingLoadingState, BriefingReadyState } from './BriefingEmptyStates';
import { BriefingWarmupState } from './BriefingWarmupState';
import { DigestView } from './DigestView';
import { CommunityInsights } from './CommunityInsights';
import { ProGate } from './ProGate';
import {
  SectionAccent,
  sectionTitleColor,
  renderLine,
} from '../utils/briefing-parser';
import { EngagementPulse } from './EngagementPulse';
import { IntelligencePulse } from './IntelligencePulse';
import { ScoringDelta } from './ScoringDelta';
import { DecisionWindowsPanel } from './DecisionWindowsPanel';
import { CompoundAdvantageScore } from './CompoundAdvantageScore';
import { IntelligenceProfileCard } from './IntelligenceProfileCard';
import { StreetsContextCard } from './StreetsContextCard';
import { GuidedMissions } from './GuidedMissions';
import { WisdomPulse } from './WisdomPulse';
import { WeeklyIntelligenceSummary } from './WeeklyIntelligenceSummary';
import { ContextualTip } from './ContextualTip';
import { useLicense } from '../hooks/use-license';
import { useBriefingDerived } from '../hooks/use-briefing-derived';
import type { SourceRelevance } from '../types';

interface StreetsSuggestionData {
  module_id: string;
  module_title: string;
  reason: string;
  match_strength: number;
}

export const BriefingView = memo(function BriefingView() {
  const { t } = useTranslation();

  // 2a. Consolidated data subscriptions with useShallow
  const {
    briefing, results, isLoading, analysisComplete, feedbackGiven,
    lastBackgroundResultsAt, sourceHealth, pulse,
    freeBriefing, freeBriefingLoading, embeddingMode,
  } = useAppStore(
    useShallow((s) => ({
      briefing: s.aiBriefing,
      results: s.appState.relevanceResults,
      isLoading: s.appState.loading,
      analysisComplete: s.appState.analysisComplete,
      feedbackGiven: s.feedbackGiven,
      lastBackgroundResultsAt: s.lastBackgroundResultsAt,
      sourceHealth: s.sourceHealth,
      pulse: s.intelligencePulse,
      freeBriefing: s.freeBriefing,
      freeBriefingLoading: s.freeBriefingLoading,
      embeddingMode: s.embeddingMode,
    })),
  );

  // Action selectors (stable references)
  const generateBriefing = useAppStore(s => s.generateBriefing);
  const recordInteraction = useAppStore(s => s.recordInteraction);
  const setActiveView = useAppStore(s => s.setActiveView);
  const addToast = useAppStore(s => s.addToast);
  const generateFreeBriefing = useAppStore(s => s.generateFreeBriefing);
  const loadPulse = useAppStore(s => s.loadIntelligencePulse);
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

  // STREETS contextual suggestion
  const [streetsSuggestion, setStreetsSuggestion] = useState<StreetsSuggestionData | null>(null);

  const [gapExpanded, setGapExpanded] = useState(false);
  const [metricsExpanded, setMetricsExpanded] = useState(false);

  // Stable callbacks for memo-wrapped BriefingCard/SignalActionCard
  const handleSave = useCallback((it: SourceRelevance) => recordInteraction(it.id, 'save', it), [recordInteraction]);
  const handleDismiss = useCallback((it: SourceRelevance) => recordInteraction(it.id, 'dismiss', it), [recordInteraction]);
  const handleRecordClick = useCallback((it: SourceRelevance) => recordInteraction(it.id, 'click', it), [recordInteraction]);

  // Load intelligence pulse from store
  useEffect(() => {
    loadPulse();
  }, [loadPulse]);

  // Listen for standing query matches from background monitoring
  useEffect(() => {
    const unlisten = listen<Array<{ query_id: number; query_text: string; new_matches: number; example_title: string | null }>>(
      'standing-query-matches',
      (event) => {
        const alerts = event.payload.filter(a => a.new_matches > 0);
        if (alerts.length > 0) {
          const msg = alerts.length === 1
            ? t('standingQueries.singleMatch', { query: alerts[0].query_text, count: alerts[0].new_matches })
            : t('standingQueries.multiMatch', { count: alerts.length });
          addToast('info', msg);
        }
      },
    );
    return () => { unlisten.then(fn => fn()); };
  }, [addToast, t]);

  // Fetch STREETS contextual suggestion on mount
  useEffect(() => {
    cmd('get_streets_suggestion')
      .then((suggestion) => {
        if (!suggestion) {
          setStreetsSuggestion(null);
          return;
        }
        // Check localStorage for 7-day dismiss
        const dismissKey = `streets_dismiss_${suggestion.module_id}`;
        const dismissedAt = localStorage.getItem(dismissKey);
        if (dismissedAt) {
          const elapsed = Date.now() - parseInt(dismissedAt, 10);
          if (elapsed < 7 * 24 * 60 * 60 * 1000) {
            setStreetsSuggestion(null);
            return;
          }
          localStorage.removeItem(dismissKey);
        }
        setStreetsSuggestion(suggestion);
      })
      .catch(() => setStreetsSuggestion(null));
  }, [analysisComplete]);

  const handleStreetsDismiss = useCallback((moduleId: string) => {
    localStorage.setItem(`streets_dismiss_${moduleId}`, Date.now().toString());
    setStreetsSuggestion(null);
  }, []);

  const handleStreetsOpen = useCallback((moduleId: string) => {
    setActiveView('playbook');
    // Small delay to let the view switch, then trigger module load
    setTimeout(() => {
      const store = useAppStore.getState();
      store.loadPlaybookContent?.(moduleId);
    }, 100);
  }, [setActiveView]);

  // Derived computations (gaps, quality, health, signals, top picks)
  const { gaps, lowQualitySources, healthSummary, sections, isStale, signalItems, topItems } =
    useBriefingDerived(results, sourceHealth, briefing, lastBackgroundResultsAt);

  // Copy raw briefing markdown
  const copyBriefing = useCallback(async () => {
    if (!briefing.content) return;
    await window.navigator.clipboard.writeText(briefing.content);
    addToast('success', t('briefing.copiedToClipboard'));
  }, [briefing.content, addToast, t]);

  // Share condensed briefing — reuses memoized sections
  const shareBriefing = useCallback(async () => {
    if (!briefing.content) return;
    const kept = sections.filter(s => s.type === 'action' || s.type === 'worth_knowing');
    const date = new Date().toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
    const lines = [`4DA Intelligence Briefing — ${date}\n`];
    for (const s of kept) {
      lines.push(`## ${s.title}`);
      lines.push(s.lines.join('\n'));
      lines.push('');
    }
    lines.push('Generated by 4DA (4da.dev)');
    await window.navigator.clipboard.writeText(lines.join('\n'));
    addToast('success', t('briefing.condensedCopied'));
  }, [briefing.content, sections, addToast, t]);

  // Auto-generate free briefing for all users when analysis completes
  useEffect(() => {
    if (analysisComplete && results.length > 0 && !freeBriefing && !freeBriefingLoading) {
      generateFreeBriefing();
    }
  }, [analysisComplete, results.length, freeBriefing, freeBriefingLoading, generateFreeBriefing]);

  // 2b. Loading skeleton with stable widths
  if (briefing.loading) {
    return (
      <div className="bg-bg-primary rounded-lg" role="status" aria-busy="true" aria-label="Loading briefing">
        <div className="space-y-4">
          {/* Skeleton header */}
          <div className="bg-bg-secondary rounded-lg border border-border p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                <div className="w-4 h-4 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
              </div>
              <div>
                <div className="h-5 w-48 bg-bg-tertiary rounded animate-pulse" />
                <div className="h-3 w-32 bg-bg-tertiary rounded animate-pulse mt-2" />
              </div>
            </div>
            {/* Skeleton lines */}
            <div className="space-y-3">
              {SKELETON_WIDTHS.map((w, i) => (
                <div key={i} className="h-4 bg-bg-tertiary rounded animate-pulse" style={{ width: `${w}%` }} />
              ))}
            </div>
          </div>
          {/* Skeleton cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="bg-bg-secondary rounded-lg border border-border p-4">
                <div className="flex gap-3">
                  <div className="w-10 h-6 bg-bg-tertiary rounded animate-pulse" />
                  <div className="flex-1 space-y-2">
                    <div className="h-4 bg-bg-tertiary rounded animate-pulse" />
                    <div className="h-3 bg-bg-tertiary rounded animate-pulse w-3/4" />
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  // Empty state: no briefing content and not generating
  if (!briefing.content) {
    if (isLoading) return <BriefingLoadingState />;

    // Free briefing for non-Pro users
    if (!isPro && freeBriefing && !freeBriefing.empty) {
      return (
        <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg space-y-4">
          {showPersonalizeNudge && (
            <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-4 flex items-start justify-between gap-3">
              <div>
                <h3 className="text-sm font-medium text-white mb-1">{t('briefing.personalizeTitle')}</h3>
                <p className="text-xs text-text-secondary mb-3">{t('briefing.personalizeBody')}</p>
                <button
                  onClick={() => setShowSettings(true)}
                  className="px-3 py-1.5 text-xs bg-blue-500/20 text-blue-400 border border-blue-500/30 rounded-lg hover:bg-blue-500/30 transition-all font-medium"
                >
                  {t('header.settings')}
                </button>
              </div>
              <button
                onClick={() => setPersonalizeCardDismissed(true)}
                className="text-text-muted hover:text-white transition-colors flex-shrink-0 p-1"
                aria-label={t('action.dismiss')}
              >
                &#x2715;
              </button>
            </div>
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
                        onClick={() => window.open(item.url!, '_blank', 'noopener,noreferrer')}
                        className="text-sm text-white hover:text-orange-400 text-left transition-colors"
                      >
                        {item.title}
                      </button>
                    ) : (
                      <span className="text-sm text-white">{item.title}</span>
                    )}
                    <span className="text-xs text-text-muted ml-2">{item.source}</span>
                  </div>
                </div>
              ))}
            </div>
            {freeBriefing.stack_alerts && freeBriefing.stack_alerts.length > 0 && (
              <div className="mt-4 pt-3 border-t border-border">
                <h3 className="text-xs font-medium text-amber-400 mb-2">{t('briefing.stackAlerts')}</h3>
                {freeBriefing.stack_alerts.map((alert, i) => (
                  <div key={i} className="text-xs text-text-secondary py-0.5">{alert.title}</div>
                ))}
              </div>
            )}
            <div className="mt-3 pt-3 border-t border-border flex items-center justify-between">
              <span className="text-xs text-text-muted">{t('briefing.itemsAnalyzed', { count: freeBriefing.total_items })}</span>
              <ProGate feature="AI Briefings">
                <button
                  onClick={generateBriefing}
                  className="px-3 py-1.5 text-xs bg-orange-500/10 text-orange-400 border border-orange-500/20 rounded-lg hover:bg-orange-500/20 transition-all font-medium"
                >
                  {t('briefing.generateAI')}
                </button>
              </ProGate>
            </div>
          </div>
          <EngagementPulse />
        </section>
      );
    }

    if (analysisComplete && results.length > 0) return <BriefingReadyState />;
    return <BriefingWarmupState onAnalyze={startAnalysis} />;
  }

  // Personalization nudge card (inline, non-blocking)
  const personalizeNudge = showPersonalizeNudge ? (
    <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-4 mb-4 flex items-start justify-between gap-3">
      <div>
        <h3 className="text-sm font-medium text-white mb-1">{t('briefing.personalizeTitle')}</h3>
        <p className="text-xs text-text-secondary mb-3">{t('briefing.personalizeBody')}</p>
        <button
          onClick={() => setShowSettings(true)}
          className="px-3 py-1.5 text-xs bg-blue-500/20 text-blue-400 border border-blue-500/30 rounded-lg hover:bg-blue-500/30 transition-all font-medium"
        >
          {t('header.settings')}
        </button>
      </div>
      <button
        onClick={() => setPersonalizeCardDismissed(true)}
        className="text-text-muted hover:text-white transition-colors flex-shrink-0 p-1"
        aria-label={t('action.dismiss')}
      >
        &#x2715;
      </button>
    </div>
  ) : null;

  // Briefing content view
  return (
    <section aria-label={t('briefing.intelligenceBriefing')} className="bg-bg-primary rounded-lg space-y-6">
      {personalizeNudge}
      <BriefingAtmosphere
        signalCount={signalItems.length}
        topCount={topItems.length}
        hasContent={!!briefing.content}
      />

      {/* 0a. Weekly Digest — self-hides when no digest available */}
      <DigestView />

      {/* 0b. Community Intelligence status — self-hides when not enabled */}
      <CommunityInsights />

      {/* 0c. AWE Wisdom Pulse — ambient wisdom layer, self-hides when empty */}
      <WisdomPulse />

      {/* 1. Decision Windows — urgency first */}
      <DecisionWindowsPanel />

      {/* 2. Signal Action Cards — critical/high priority items */}
      {signalItems.length > 0 && (
        <div className="space-y-3">
          {signalItems.map(item => (
            <SignalActionCard
              key={item.id}
              item={item}
              feedbackGiven={feedbackGiven[item.id]}
              onSave={handleSave}
              onDismiss={handleDismiss}
            />
          ))}
        </div>
      )}

      {/* 3. Learning Narrative Banner — one sentence from autophagy */}
      {pulse?.learning_narratives?.[0] && (
        <div className="bg-bg-secondary border border-border rounded-lg px-4 py-2.5 flex items-center gap-3">
          <span className="text-[10px] text-text-muted uppercase tracking-wider shrink-0">{t('briefing.systemLearned')}</span>
          <p className="text-xs text-white">{pulse.learning_narratives[0]}</p>
        </div>
      )}

      {/* 4. Top items as BriefingCards — immediate value */}
      {topItems.length > 0 && (
        <div>
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-sm font-medium text-white">{t('briefing.topPicks')}</h3>
            <span className="text-xs text-text-muted">{t('briefing.itemCount', { count: topItems.length })}</span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {topItems.map(item => {
              const hasWorkMatch = item.score_breakdown?.intent_boost && item.score_breakdown.intent_boost > 0;
              const hasDep = item.score_breakdown?.dep_match_score && item.score_breakdown.dep_match_score > 0;
              const matchedDeps = item.score_breakdown?.matched_deps;
              return (
                <div key={item.id} className="relative">
                  {(hasWorkMatch || hasDep) && (
                    <div className="flex items-center gap-1.5 mb-1.5">
                      {hasWorkMatch && (
                        <span className="text-[10px] px-1.5 py-0.5 bg-purple-500/10 text-purple-400 border border-purple-500/20 rounded font-medium">
                          {t('briefing.workingOn')}
                        </span>
                      )}
                      {hasDep && (
                        <span className="text-[10px] px-1.5 py-0.5 bg-blue-500/10 text-blue-400 border border-blue-500/20 rounded font-medium">
                          {matchedDeps ? t('briefing.stackDeps', { deps: matchedDeps.slice(0, 3).join(', ') }) : t('briefing.stack')}
                        </span>
                      )}
                    </div>
                  )}
                  <BriefingCard
                    item={item}
                    explanation={item.explanation}
                    feedbackGiven={feedbackGiven[item.id]}
                    onSave={handleSave}
                    onDismiss={handleDismiss}
                    onRecordInteraction={handleRecordClick}
                  />
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* 5. Briefing content */}
      <div className="bg-bg-secondary rounded-lg border border-orange-500/20 overflow-hidden">
        <div className="px-5 py-4 border-b border-orange-500/10 flex items-center justify-between bg-orange-500/5">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
              <span className="text-orange-400 text-sm">*</span>
            </div>
            <div>
              <h2 className="font-medium text-orange-400">{t('briefing.intelligenceBriefing')}</h2>
              {healthSummary && (
                <div className="flex items-center gap-1.5 mt-0.5">
                  <span className={`inline-block w-1.5 h-1.5 rounded-full ${healthSummary.allHealthy ? 'bg-green-400' : 'bg-amber-400'}`} aria-hidden="true" />
                  <span className="sr-only">{healthSummary.allHealthy ? 'All sources healthy' : 'Some sources degraded'}</span>
                  <span className={`text-[11px] ${healthSummary.allHealthy ? 'text-green-400/70' : 'text-amber-400/70'}`}>
                    {t('briefing.sourcesHealth', { healthy: healthSummary.healthy, total: healthSummary.total })}
                  </span>
                </div>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2">
            {/* 2d. Isolated tick timer — only this component re-renders every 60s */}
            {briefing.lastGenerated && (
              <RelativeTimestamp date={briefing.lastGenerated} />
            )}
            <button
              onClick={copyBriefing}
              className="px-2.5 py-1.5 text-xs bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-white hover:border-[#3A3A3A] transition-all"
              title={t('briefing.copyTooltip')}
              aria-label={t('briefing.copyTooltip')}
            >
              {t('action.copy')}
            </button>
            <button
              onClick={shareBriefing}
              className="px-2.5 py-1.5 text-xs bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-white hover:border-[#3A3A3A] transition-all"
              title={t('briefing.shareTooltip')}
              aria-label={t('briefing.shareTooltip')}
            >
              {t('briefing.share')}
            </button>
            {isPro && (
              <button
                onClick={generateBriefing}
                className="px-3 py-1.5 text-xs bg-bg-tertiary text-orange-400 border border-orange-500/30 rounded-lg hover:bg-orange-500/10 transition-all font-medium"
                title={t('briefing.refreshTooltip')}
                aria-label={t('briefing.refreshTooltip')}
              >
                {t('action.refresh')}
              </button>
            )}
          </div>
        </div>

        {/* Stale briefing indicator */}
        {isStale && (
          <div className="px-5 py-2.5 bg-yellow-500/5 border-b border-yellow-500/10 flex items-center justify-between">
            <span className="text-xs text-yellow-400">{t('briefing.staleNotice')}</span>
            <button
              onClick={generateBriefing}
              className="text-xs text-yellow-400 hover:text-yellow-300 underline font-medium"
              aria-label="Refresh stale briefing"
            >
              {t('action.refresh')}
            </button>
          </div>
        )}

        {/* Intelligence gap banner */}
        {gaps.length > 0 && (
          <div className="px-5 py-2.5 bg-amber-500/5 border-b border-amber-500/10">
            <button
              onClick={() => setGapExpanded(!gapExpanded)}
              className="w-full flex items-center justify-between text-left"
              aria-expanded={gapExpanded}
              aria-label={`${gaps.length} source${gaps.length > 1 ? 's' : ''} offline`}
            >
              <span className="text-xs text-amber-400">
                {gaps.length} source{gaps.length > 1 ? 's' : ''} offline: {gaps.map(g => g.gap_message).join(', ')}
              </span>
              <span className="text-xs text-amber-500 ml-2 flex-shrink-0" aria-hidden="true">{gapExpanded ? '\u25B2' : '\u25BC'}</span>
            </button>
            {gapExpanded && (
              <div className="mt-2 space-y-1">
                {sourceHealth.map(s => (
                  <div key={s.source_type} className="flex items-center justify-between text-xs py-0.5">
                    <span className={s.status === 'healthy' ? 'text-green-400' : 'text-amber-400'}>
                      {s.source_type}
                    </span>
                    <span className="text-text-muted">
                      {s.status === 'healthy'
                        ? `${s.items_fetched} items${s.last_success_relative ? ` \u00B7 ${s.last_success_relative}` : ''}`
                        : s.status === 'circuit_open' ? 'circuit open' : 'error'}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Source quality suggestions */}
        {lowQualitySources.length > 0 && (
          <div className="px-5 py-2.5 bg-purple-500/5 border-b border-purple-500/10">
            {lowQualitySources.map(s => (
              <div key={s.source} className="flex items-center justify-between text-xs py-0.5">
                <span className="text-purple-400">
                  {s.source}: {s.ratio}% of {s.total} items relevant to you
                </span>
                <button
                  onClick={() => setActiveView('calibrate')}
                  className="text-purple-400/70 hover:text-purple-300 transition-colors"
                  aria-label={`Review source quality for ${s.source}`}
                >
                  {t('briefing.reviewSource', 'Review')}
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Parsed sections */}
        <div className="p-5 space-y-6">
          {sections.map((section, sIdx) => (
            <div key={sIdx} className="flex gap-3">
              <SectionAccent type={section.type} />
              <div className="flex-1 min-w-0">
                <h3 className={`text-sm font-medium mb-2 ${sectionTitleColor(section.type)}`}>
                  {section.title}
                </h3>
                <div>
                  {section.lines.map((line, lIdx) => renderLine(line, lIdx, section.type))}
                </div>
              </div>
            </div>
          ))}
        </div>

        {briefing.lastGenerated && (
          <div className="px-5 py-3 border-t border-border text-xs text-text-muted">
            {t('briefing.generatedAt', { time: briefing.lastGenerated.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) })}
            {briefing.model && <span className="ml-2">{t('briefing.viaModel', { model: briefing.model })}</span>}
          </div>
        )}
      </div>

      {/* 6. Your Intelligence Profile — learning visibility */}
      <IntelligenceProfileCard />

      {/* 6b. STREETS Contextual Suggestion — surfaces relevant playbook modules */}
      {streetsSuggestion && (
        <StreetsContextCard
          suggestion={streetsSuggestion}
          onOpen={handleStreetsOpen}
          onDismiss={handleStreetsDismiss}
        />
      )}

      {/* Weekly intelligence summary — shows once per 7 days */}
      <WeeklyIntelligenceSummary />

      {/* Guided missions — first 48h onboarding */}
      <GuidedMissions />

      {/* Contextual tip: teach feedback loop */}
      <ContextualTip
        tipId="feedback-loop"
        message={t('tips.feedbackLoop', 'Save articles you find useful — this teaches the system what matters to you.')}
        hint={t('tips.feedbackLoopHint', 'Dismissing articles also helps. Every interaction improves your results.')}
        showWhen={Object.keys(feedbackGiven).length === 0 && results.length > 0}
      />

      {/* Contextual tip: Ollama nudge for keyword-only users */}
      <ContextualTip
        tipId="ollama-nudge"
        message={t('tips.ollamaNudge', 'Your results use keyword matching. Ollama (free, local) unlocks semantic matching for more relevant results.')}
        hint={t('tips.ollamaNudgeHint', 'Install Ollama, then enable it in Settings > AI Provider. Runs entirely on your machine.')}
        showWhen={embeddingMode === 'keyword-only' && results.length > 0}
      />

      {/* 7. Intelligence Metrics — 2c. conditionally mounted when expanded */}
      <div>
        <button
          onClick={() => setMetricsExpanded(prev => !prev)}
          aria-expanded={metricsExpanded}
          aria-label={t('briefing.intelligenceMetrics')}
          className="flex items-center gap-2 text-xs text-text-muted cursor-pointer py-2 w-full text-left"
        >
          <span>{t('briefing.intelligenceMetrics')}</span>
          <span className="text-[10px] bg-white/5 px-1.5 py-0.5 rounded">
            {pulse?.calibration_accuracy != null ? `${(pulse.calibration_accuracy * 100).toFixed(0)}% accuracy` : '\u2014'}
          </span>
          <span className={`ml-auto text-[10px] transition-transform duration-200 ${metricsExpanded ? 'rotate-90' : ''}`} aria-hidden="true">{'\u25B8'}</span>
        </button>
        {metricsExpanded && (
          <div className="space-y-3 pt-2">
            <EngagementPulse />
            <IntelligencePulse />
            <ScoringDelta />
            <CompoundAdvantageScore />
          </div>
        )}
      </div>

      {/* View all results link */}
      <div className="flex justify-center pt-2 pb-4">
        <button
          onClick={() => setActiveView('results')}
          className="px-6 py-2.5 text-sm text-orange-400 bg-bg-secondary border border-orange-500/20 rounded-lg hover:bg-orange-500/10 hover:border-orange-500/30 transition-all font-medium"
        >
          {t('briefing.viewAllResults', { count: results.length })}
        </button>
      </div>

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
