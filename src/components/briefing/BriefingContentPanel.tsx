import { memo, useState, useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../../lib/commands';
import { DigestView } from '../DigestView';
import { CommunityInsights } from '../CommunityInsights';
import { DecisionWindowsPanel } from '../DecisionWindowsPanel';
import { SignalActionCard } from './SignalActionCard';
import { RelativeTimestamp } from './BriefingHelpers';
import { IntelligenceProfileCard } from '../IntelligenceProfileCard';
import {
  parseBriefingContent,
  SectionAccent,
  sectionTitleColor,
  renderLine,
} from '../../utils/briefing-parser';
import { BriefingTopPicks } from './BriefingTopPicks';
import { BriefingMetrics } from './BriefingMetrics';
import type { SourceRelevance, SourceHealthStatus, FeedbackAction } from '../../types';
import type { BriefingState, ToastType } from '../../store/types';
import type { IntelligencePulseData } from '../../types/autophagy';

type ActiveView = 'briefing' | 'results' | 'saved' | 'insights' | 'toolkit' | 'playbook' | 'channels' | 'profile' | 'calibrate' | 'console';

interface BriefingContentPanelProps {
  briefing: BriefingState;
  results: SourceRelevance[];
  feedbackGiven: Record<number, FeedbackAction>;
  sourceHealth: SourceHealthStatus[];
  pulse: IntelligencePulseData | null;
  isStale: boolean;
  isPro: boolean;
  signalItems: SourceRelevance[];
  topItems: SourceRelevance[];
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
  onRecordClick: (item: SourceRelevance) => void;
  generateBriefing: () => Promise<void>;
  setActiveView: (view: ActiveView) => void;
  addToast: (type: ToastType, message: string) => void;
}

export const BriefingContentPanel = memo(function BriefingContentPanel({
  briefing,
  results,
  feedbackGiven,
  sourceHealth,
  pulse,
  isStale,
  isPro,
  signalItems,
  topItems,
  onSave,
  onDismiss,
  onRecordClick,
  generateBriefing,
  setActiveView,
  addToast,
}: BriefingContentPanelProps) {
  const { t } = useTranslation();

  const [gapExpanded, setGapExpanded] = useState(false);

  // Standing query proactive nudge
  const [topEngagedTopic, setTopEngagedTopic] = useState<{ topic: string; count: number } | null>(null);
  const [nudgeDismissed, setNudgeDismissed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    Promise.allSettled([
      cmd('ace_get_topic_affinities'),
      cmd('list_standing_queries'),
    ]).then(([affinitiesResult, queriesResult]) => {
      if (cancelled) return;
      if (affinitiesResult.status !== 'fulfilled') return;
      const affinities = affinitiesResult.value?.affinities ?? [];
      const rawQueries = queriesResult.status === 'fulfilled' ? queriesResult.value : [];
      const queries = Array.isArray(rawQueries) ? rawQueries : [];
      const queryTopics = new Set(queries.map((q: { query_text: string }) => q.query_text?.toLowerCase?.() ?? ''));
      // Find the top engaged topic that doesn't already have a standing query
      const candidate = affinities
        .filter((a: { topic: string; positive_signals: number }) =>
          a.positive_signals >= 3 && !queryTopics.has(a.topic.toLowerCase()))
        .sort((a: { positive_signals: number }, b: { positive_signals: number }) =>
          b.positive_signals - a.positive_signals)[0];
      if (candidate) {
        setTopEngagedTopic({ topic: candidate.topic, count: candidate.positive_signals });
      }
    });
    return () => { cancelled = true; };
  }, []);

  const handleCreateWatch = useCallback(async (topic: string) => {
    try {
      await cmd('create_standing_query', { queryText: topic });
      setTopEngagedTopic(null);
    } catch {
      // Non-critical — silently fail
    }
  }, []);

  // Analysis narration — live feed of what 4DA is doing
  const [narration, setNarration] = useState<string | null>(null);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const unlisten = listen<{ narration_type: string; message: string }>('analysis-narration', (event) => {
      setNarration(event.payload?.message || null);
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => setNarration(null), 3000);
    });
    return () => {
      unlisten.then(fn => fn());
      if (timer) clearTimeout(timer);
    };
  }, []);

  // Intelligence gaps -- non-healthy sources
  const gaps = useMemo(
    () => sourceHealth.filter(s => s.status !== 'healthy' && s.gap_message),
    [sourceHealth],
  );

  // Source quality analysis -- flag sources with < 5% relevance ratio
  const lowQualitySources = useMemo(() => {
    if (results.length < 10) return [];
    const bySource: Record<string, { total: number; relevant: number }> = {};
    for (const r of results) {
      const src = r.source_type ?? 'unknown';
      if (!bySource[src]) bySource[src] = { total: 0, relevant: 0 };
      bySource[src].total++;
      if (r.relevant) bySource[src].relevant++;
    }
    return Object.entries(bySource)
      .filter(([, stats]) => stats.total >= 5 && (stats.relevant / stats.total) < 0.05)
      .map(([source, stats]) => ({
        source,
        total: stats.total,
        relevant: stats.relevant,
        ratio: Math.round((stats.relevant / stats.total) * 100),
      }));
  }, [results]);

  // Source health summary for header badge
  const healthSummary = useMemo(() => {
    if (sourceHealth.length === 0) return null;
    const healthy = sourceHealth.filter(s => s.status === 'healthy').length;
    const total = sourceHealth.length;
    return { healthy, total, allHealthy: healthy === total };
  }, [sourceHealth]);

  // Memoized sections
  const sections = useMemo(() => {
    if (!briefing.content) return [];
    return parseBriefingContent(briefing.content);
  }, [briefing.content]);

  // Copy raw briefing markdown
  const copyBriefing = useCallback(async () => {
    if (!briefing.content) return;
    await window.navigator.clipboard.writeText(briefing.content);
    addToast('success', t('briefing.copiedToClipboard'));
  }, [briefing.content, addToast, t]);

  // Share condensed briefing
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

  return (
    <>
      {/* Analysis narration — live feed of what 4DA is analyzing */}
      {narration && (
        <div className="px-4 py-2 text-xs text-text-muted/70 flex items-center gap-2 animate-pulse">
          <div className="w-1.5 h-1.5 rounded-full bg-accent-gold flex-shrink-0" />
          {narration}
        </div>
      )}

      {/* 0a. Weekly Digest */}
      <DigestView />

      {/* 0b. Community Intelligence status */}
      <CommunityInsights />

      {/* 1. Decision Windows */}
      <DecisionWindowsPanel />

      {/* 2. Signal Action Cards */}
      {signalItems.length > 0 && (
        <div className="space-y-3">
          {signalItems.map(item => (
            <SignalActionCard
              key={item.id}
              item={item}
              feedbackGiven={feedbackGiven[item.id]}
              onSave={onSave}
              onDismiss={onDismiss}
            />
          ))}
        </div>
      )}

      {/* 3. Learning Narrative Banner */}
      {pulse?.learning_narratives?.[0] && (
        <div className="bg-bg-secondary border border-border rounded-lg px-4 py-2.5 flex items-center gap-3">
          <span className="text-[10px] text-text-muted uppercase tracking-wider shrink-0">{t('briefing.systemLearned')}</span>
          <p className="text-xs text-white">{pulse.learning_narratives[0]}</p>
        </div>
      )}

      {/* 4. Top items as BriefingCards */}
      <BriefingTopPicks
        topItems={topItems}
        feedbackGiven={feedbackGiven}
        onSave={onSave}
        onDismiss={onDismiss}
        onRecordClick={onRecordClick}
      />

      {/* 4b. Standing query proactive nudge */}
      {topEngagedTopic && !nudgeDismissed && (
        <div className="bg-bg-tertiary rounded-lg border border-border/30 p-3">
          <p className="text-xs text-text-secondary">
            You've engaged with {topEngagedTopic.count} articles about{' '}
            <span className="text-text-primary font-medium">{topEngagedTopic.topic}</span> recently.
          </p>
          <div className="flex items-center gap-3 mt-1.5">
            <button
              onClick={() => handleCreateWatch(topEngagedTopic.topic)}
              className="text-xs text-accent-gold hover:text-white transition-colors"
            >
              Watch for more?
            </button>
            <button
              onClick={() => setNudgeDismissed(true)}
              className="text-xs text-text-muted hover:text-text-secondary transition-colors"
            >
              Dismiss
            </button>
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

      {/* 6. Your Intelligence Profile */}
      <IntelligenceProfileCard />

      {/* 7. Intelligence Metrics */}
      <BriefingMetrics pulse={pulse} />

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
    </>
  );
});
