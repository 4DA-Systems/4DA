// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

import { BrandMark } from '../void-engine/BrandMark';
import { getCelebrationMessage } from '../../utils/first-run-messages';
import { getSourceFullName } from '../../config/sources';
import { getRelevancePresentation } from '../../utils/score';
import { useAppStore } from '../../store';

interface TopSignal {
  title: string;
  url: string | null;
  top_score?: number;
  source_type?: string;
  score_breakdown?: {
    dep_match_score?: number;
    matched_deps?: string[];
    skill_gap_boost?: number;
    stack_boost?: number;
    domain_relevance?: number;
  };
}

interface CelebrationStateProps {
  relevantCount: number;
  totalCount: number;
  sourceBreakdown: Array<[string, number]>;
  topSignal: TopSignal | null;
  stackInsights: string[];
  embeddingMode: string | null;
  detectedTech?: Array<{ name: string; category: string; confidence: number }>;
  /** True when there is no profile signal to rank against (setup skipped, no
   *  project folder, no ACE context). 0 relevant is then structural, not a
   *  failure — show an honest "add a signal" framing, not a vanity "0". */
  profileEmpty?: boolean;
  onDismiss: (view: 'briefing' | 'results') => void;
}

/** Build a human-readable insight explaining why the top signal matched */
function buildMatchReason(signal: TopSignal, t: (key: string, opts?: Record<string, unknown>) => string): string | null {
  const parts: string[] = [];
  const sb = signal.score_breakdown;
  if (!sb) return null;

  if (sb.matched_deps && sb.matched_deps.length > 0) {
    parts.push(t('firstRun.matchesDependencies', { deps: sb.matched_deps.slice(0, 4).join(', ') }));
  }
  if (sb.stack_boost && sb.stack_boost > 0) {
    parts.push(t('firstRun.relevantToStack'));
  }
  if (sb.skill_gap_boost && sb.skill_gap_boost > 0) {
    parts.push(t('firstRun.coversGap'));
  }

  return parts.length > 0 ? parts.join(' \u00b7 ') : null;
}

export function CelebrationState({
  relevantCount,
  totalCount,
  sourceBreakdown,
  topSignal,
  stackInsights,
  embeddingMode,
  detectedTech,
  profileEmpty = false,
  onDismiss,
}: CelebrationStateProps) {
  const { t } = useTranslation();
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const setSettingsInitialTab = useAppStore(s => s.setSettingsInitialTab);
  const matchReason = topSignal ? buildMatchReason(topSignal, t) : null;

  // Deep-link straight to the Projects tab (folders + stack discovery) — not the
  // General tab — then drop into results so Settings is the visible surface.
  const handleAddSignal = () => {
    setSettingsInitialTab('projects');
    setShowSettings(true);
    onDismiss('results');
  };

  // Count items matching active dependencies
  const depMatchCount = sourceBreakdown.reduce((sum, [, count]) => sum + count, 0);

  return (
    <div className="text-center px-8 max-w-lg">
      <div className="mb-6">
        <BrandMark size={80} />
      </div>

      {/* Intelligence Activated header */}
      <h2 className="text-lg font-medium text-text-primary mb-4">
        {t('firstRun.intelligenceActivated', 'Intelligence Activated')}
      </h2>

      {/* Stats row */}
      <div className="flex justify-center gap-8 mb-6">
        <div className="text-center">
          <span className="text-3xl font-bold text-text-primary tabular-nums">{totalCount}</span>
          <p className="text-[10px] text-text-muted uppercase tracking-wider mt-0.5">
            {t('firstRun.itemsAnalyzed', 'analyzed')}
          </p>
        </div>
        {/* "Relevant" is hidden on a profileless first run — showing a giant
            "0" is a banned vanity zero (doctrine rule 3) and reads as failure. */}
        {!profileEmpty && (
          <div className="text-center">
            <span className="text-3xl font-bold text-text-primary tabular-nums">{relevantCount}</span>
            <p className="text-[10px] text-text-muted uppercase tracking-wider mt-0.5">
              {t('firstRun.relevantToYou', 'relevant')}
            </p>
          </div>
        )}
        {((profileEmpty && sourceBreakdown.length > 0) || (depMatchCount > 0 && depMatchCount !== totalCount)) && (
          <div className="text-center">
            <span className="text-3xl font-bold text-text-primary tabular-nums">{sourceBreakdown.length}</span>
            <p className="text-[10px] text-text-muted uppercase tracking-wider mt-0.5">
              {t('firstRun.sourcesScanned', 'sources')}
            </p>
          </div>
        )}
      </div>

      {/* Developer DNA Preview — first "it knows me" moment */}
      {detectedTech && detectedTech.length > 0 && (
        <div className="mb-6 max-w-sm mx-auto">
          <p className="text-[10px] text-text-muted uppercase tracking-wider mb-2 text-start">
            {t('firstRun.yourDna', 'Your Developer DNA')}
          </p>
          <div className="flex flex-wrap gap-1.5 justify-center">
            {detectedTech
              .filter(tech => tech.confidence >= 0.5)
              .slice(0, 12)
              .map(tech => (
                <span
                  key={tech.name}
                  className={`px-2 py-1 text-xs rounded-lg border ${
                    tech.category === 'Language'
                      ? 'bg-blue-500/10 text-blue-400 border-blue-500/20'
                      : tech.category === 'Framework'
                      ? 'bg-purple-500/10 text-purple-400 border-purple-500/20'
                      : 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
                  }`}
                >
                  {tech.name}
                </span>
              ))}
          </div>
          <p className="text-[10px] text-text-muted mt-2 italic">
            {t('firstRun.dnaHint', "We're now watching for relevant content about your stack")}
          </p>
        </div>
      )}

      {/* Celebration message */}
      <p className="text-sm text-text-secondary mb-6">
        {getCelebrationMessage(relevantCount, totalCount, profileEmpty)}
      </p>

      {/* Top signal highlight with match reasoning */}
      {topSignal && (
        <div className="mb-6 p-4 bg-bg-secondary rounded-lg border border-orange-500/20 text-start max-w-sm mx-auto">
          <p className="text-[10px] text-orange-400 font-medium uppercase tracking-wider mb-1">
            {topSignal.score_breakdown?.dep_match_score && topSignal.score_breakdown.dep_match_score > 0
              ? t('firstRun.topMatchStack', 'Matches your stack')
              : t('firstRun.topMatch')}
            {topSignal.top_score != null && (
              <span className={`ms-2 normal-case ${getRelevancePresentation(topSignal.top_score).colorClass}`}>{t(getRelevancePresentation(topSignal.top_score).labelKey)}</span>
            )}
            {topSignal.source_type && (
              <span className="ms-2 text-text-muted normal-case">{getSourceFullName(topSignal.source_type)}</span>
            )}
          </p>
          <p className="text-sm text-text-primary font-medium leading-snug line-clamp-2">{topSignal.title}</p>
          {topSignal.score_breakdown?.matched_deps && topSignal.score_breakdown.matched_deps.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-2">
              {topSignal.score_breakdown.matched_deps.slice(0, 4).map(dep => (
                <span key={dep} className="px-1.5 py-0.5 text-[10px] bg-blue-500/10 text-blue-400 rounded">
                  {dep}
                </span>
              ))}
            </div>
          )}
          {matchReason && (
            <p className="text-[10px] text-text-secondary mt-2 italic">{matchReason}</p>
          )}
          <p className="text-xs text-text-muted mt-1 truncate">{topSignal.url}</p>
        </div>
      )}

      {/* Stack-specific insights */}
      {stackInsights.length > 0 && (
        <div className="mb-6 space-y-2 max-w-sm mx-auto">
          <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1 text-start">
            {t('firstRun.stackInsight', 'Stack Insight')}
          </p>
          {stackInsights.slice(0, 3).map((insight, i) => (
            <div key={i} className="px-4 py-2.5 bg-bg-secondary rounded-lg border border-border text-start">
              <p className="text-xs text-text-secondary leading-relaxed">{insight}</p>
            </div>
          ))}
        </div>
      )}

      {/* Source breakdown */}
      {sourceBreakdown.length > 0 && (
        <div className="flex flex-wrap justify-center gap-2 mb-6">
          {sourceBreakdown.map(([src, count]) => (
            <span key={src} className="px-2.5 py-1 text-xs bg-bg-secondary text-text-secondary rounded-lg border border-border">
              {getSourceFullName(src)} <span className="text-text-muted">{count}</span>
            </span>
          ))}
        </div>
      )}

      {/* Keyword-only mode indicator */}
      {embeddingMode === 'keyword-only' && (
        <div className="mb-6 px-4 py-3 bg-amber-500/10 border border-amber-500/30 rounded-lg max-w-sm mx-auto text-start">
          <p className="text-xs font-medium text-amber-400">
            {t('firstRun.keywordModeActive', 'Running on keyword matching')}
          </p>
          <p className="text-[11px] text-amber-400/70 mt-1">
            {t('firstRun.keywordModeHint', 'Add an AI provider in Settings to unlock semantic understanding — finds content keywords alone would miss.')}
          </p>
        </div>
      )}

      {/* CTAs — a profileless first run leads with the one action that unlocks
          ranking (add a signal), then offers the fresh picks. */}
      <div className="flex flex-col items-center gap-3">
        {profileEmpty ? (
          <>
            <button
              onClick={handleAddSignal}
              aria-label={t('firstRun.addSignalAria', 'Add your stack or a project folder in Settings')}
              className="px-8 py-3 bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 hover:scale-105 active:scale-95 transition-all"
            >
              {t('firstRun.addSignalCta', 'Add your stack to start ranking')}
            </button>
            <button
              onClick={() => onDismiss('results')}
              aria-label={t('firstRun.ariaBrowseResults', { count: totalCount })}
              className="text-sm text-text-muted hover:text-text-secondary transition-colors"
            >
              {t('firstRun.browseFreshPicks', 'Browse the fresh picks')}
            </button>
          </>
        ) : (
          <>
            <button
              onClick={() => onDismiss('briefing')}
              aria-label={t('firstRun.ariaSeeBriefing')}
              className="px-8 py-3 bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 hover:scale-105 active:scale-95 transition-all"
            >
              {t('firstRun.seeBriefing')}
            </button>
            <button
              onClick={() => onDismiss('results')}
              aria-label={t('firstRun.ariaBrowseResults', { count: totalCount })}
              className="text-sm text-text-muted hover:text-text-secondary transition-colors"
            >
              {t('firstRun.browseResults', { count: totalCount })}
            </button>
          </>
        )}
      </div>

    </div>
  );
}
