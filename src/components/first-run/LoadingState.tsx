import { useEffect, useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { VoidEngine } from '../void-engine/VoidEngine';
import { getStageNarration } from '../../utils/first-run-messages';
import { registerGameComponent } from '../../lib/game-components';
import type { Phase, ScanSummary } from './utils';

// ============================================================================
// Intelligence Preview — "Here's what I found" interstitial
// ============================================================================

function IntelligencePreview({ summary }: { summary: ScanSummary }) {
  const { t } = useTranslation();

  // Build ecosystem breakdown pills
  const ecosystems = [
    summary.dependencies_by_ecosystem.rust > 0 && { label: 'Rust', count: summary.dependencies_by_ecosystem.rust },
    summary.dependencies_by_ecosystem.npm > 0 && { label: 'npm', count: summary.dependencies_by_ecosystem.npm },
    summary.dependencies_by_ecosystem.python > 0 && { label: 'Python', count: summary.dependencies_by_ecosystem.python },
    summary.dependencies_by_ecosystem.other > 0 && { label: 'Other', count: summary.dependencies_by_ecosystem.other },
  ].filter(Boolean) as { label: string; count: number }[];

  return (
    <div className="text-center px-8 max-w-md animate-fade-in">
      <div className="mb-6">
        <VoidEngine size={80} />
      </div>

      <h2 className="text-xl font-medium text-white mb-2">
        {t('firstRun.intelligenceTitle', "Here's what I found")}
      </h2>

      {/* Scan stats */}
      <div className="flex justify-center gap-6 mb-5">
        <div className="text-center">
          <span className="text-2xl font-bold text-white tabular-nums">{summary.projects_scanned}</span>
          <p className="text-[10px] text-text-muted uppercase tracking-wider mt-0.5">
            {t('firstRun.projects', 'projects')}
          </p>
        </div>
        <div className="text-center">
          <span className="text-2xl font-bold text-white tabular-nums">{summary.total_dependencies}</span>
          <p className="text-[10px] text-text-muted uppercase tracking-wider mt-0.5">
            {t('firstRun.dependencies', 'dependencies')}
          </p>
        </div>
      </div>

      {/* Primary stack */}
      {summary.primary_stack && (
        <div className="mb-5 px-4 py-3 bg-bg-secondary rounded-lg border border-border">
          <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
            {t('firstRun.primaryStack', 'Primary stack')}
          </p>
          <p className="text-sm text-white font-medium">{summary.primary_stack}</p>
        </div>
      )}

      {/* Ecosystem breakdown */}
      {ecosystems.length > 0 && (
        <div className="flex flex-wrap justify-center gap-2 mb-5">
          {ecosystems.map(({ label, count }) => (
            <span key={label} className="px-2.5 py-1 text-xs bg-bg-secondary text-text-secondary rounded-lg border border-border">
              {label} <span className="text-text-muted">{count}</span>
            </span>
          ))}
        </div>
      )}

      {/* What I'll watch for */}
      {summary.key_packages.length > 0 && (
        <div className="mb-5 px-4 py-3 bg-bg-secondary rounded-lg border border-orange-500/10 text-left">
          <p className="text-[10px] text-orange-400 font-medium uppercase tracking-wider mb-2">
            {t('firstRun.watchingFor', "I'll watch for")}
          </p>
          <p className="text-xs text-text-secondary leading-relaxed">
            {t('firstRun.watchingDescription', {
              packages: summary.key_packages.slice(0, 5).join(', '),
              defaultValue: `Security advisories, breaking changes, and updates for ${summary.key_packages.slice(0, 5).join(', ')}`,
            })}
          </p>
        </div>
      )}

      {/* Loading indicator */}
      <p className="text-xs text-text-muted animate-pulse">
        {t('firstRun.startingAnalysis', 'Starting content analysis...')}
      </p>
    </div>
  );
}

// ============================================================================
// Loading State — preparing, intelligence, fetching, analyzing phases
// ============================================================================

// ============================================================================
// Narration Feed — live analysis narration events
// ============================================================================

interface NarrationFeedEvent {
  type: string;
  message: string;
  timestamp: number;
}

function NarrationFeed({ events }: { events: NarrationFeedEvent[] }) {
  const visible = events.slice(-6);
  if (visible.length === 0) return null;

  return (
    <div className="space-y-1.5 max-h-40 overflow-hidden max-w-xs mx-auto mt-4">
      {visible.map((event, i) => (
        <div
          key={event.timestamp}
          className="text-xs text-text-secondary animate-fade-in"
          style={{ opacity: i === visible.length - 1 ? 1 : 0.5 + (i * 0.1) }}
        >
          {event.message}
        </div>
      ))}
    </div>
  );
}

interface LoadingStateProps {
  phase: Phase;
  progress: number;
  progressStage: string;
  itemCount: number;
  sourceMessages: string[];
  interests: string[];
  embeddingMode: string | null;
  scanSummary: ScanSummary | null;
  narrationEvents?: NarrationFeedEvent[];
}

export function LoadingState({
  phase,
  progress,
  progressStage,
  itemCount,
  sourceMessages,
  interests,
  embeddingMode,
  scanSummary,
  narrationEvents,
}: LoadingStateProps) {
  const { t } = useTranslation();

  useEffect(() => { registerGameComponent('game-boot-ring'); }, []);

  // Estimated time remaining — starts at 90s, decrements every second
  // Uses a ref-based counter to avoid infinite timer chains in test environments
  const [estimatedSeconds, setEstimatedSeconds] = useState(90);
  const counterRef = useRef(90);

  useEffect(() => {
    if (phase === 'intelligence' || phase === 'celebrating' || phase === 'fading') return;

    const interval = setInterval(() => {
      if (counterRef.current <= 0) {
        clearInterval(interval);
        return;
      }
      counterRef.current -= 1;
      setEstimatedSeconds(counterRef.current);
    }, 1000);

    return () => clearInterval(interval);
  }, [phase]);

  // Intelligence preview phase
  if (phase === 'intelligence' && scanSummary) {
    return <IntelligencePreview summary={scanSummary} />;
  }

  return (
    <div className="text-center px-8 max-w-md">
      <div className="mb-6">
        <VoidEngine size={80} />
      </div>

      {phase === 'preparing' && (
        <div className="w-10 h-10 mx-auto mb-2 opacity-60">
          <game-boot-ring style={{ width: '40px', height: '40px' }} />
        </div>
      )}

      <h2 className="text-xl font-medium text-white mb-2">
        {phase === 'preparing' && t('firstRun.preparing')}
        {phase === 'fetching' && t('firstRun.fetching')}
        {phase === 'analyzing' && t('firstRun.analyzing')}
      </h2>

      {/* Stage narration */}
      <p className="text-sm text-text-secondary mb-6">
        {getStageNarration(progressStage || 'init')}
      </p>

      {/* Estimated time remaining */}
      {(phase === 'preparing' || phase === 'fetching' || phase === 'analyzing') && (
        <p className="text-sm text-text-secondary mb-4" aria-live="polite">
          {estimatedSeconds <= 0
            ? t('firstRun.estimatedTimeAlmost', 'Almost there...')
            : estimatedSeconds < 30
              ? t('firstRun.estimatedTimeLow', '< 30s remaining')
              : t('firstRun.estimatedTime', { seconds: estimatedSeconds, defaultValue: '~{{seconds}}s remaining' })
          }
        </p>
      )}

      {/* User interests (preparing phase) */}
      {phase === 'preparing' && interests.length > 0 && (
        <div className="flex flex-wrap justify-center gap-2 mb-6">
          {interests.map(topic => (
            <span key={topic} className="px-2.5 py-1 text-xs bg-orange-500/10 text-orange-400 rounded-lg border border-orange-500/20">
              {topic}
            </span>
          ))}
        </div>
      )}

      {/* Progress bar (fetching/analyzing) */}
      {(phase === 'fetching' || phase === 'analyzing') && (
        <div className="max-w-xs mx-auto mb-4">
          <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-500 ease-out rounded-full"
              style={{ width: `${Math.max(progress * 100, 5)}%` }}
            />
          </div>
          <div className="flex justify-between text-xs text-text-muted mt-1.5">
            <span>{itemCount > 0 ? t('firstRun.itemsFound', { count: itemCount }) : ''}</span>
            <span>{Math.round(progress * 100)}%</span>
          </div>
        </div>
      )}

      {/* Source-by-source narration (fetching phase) */}
      {phase === 'fetching' && sourceMessages.length > 0 && (
        <div className="space-y-1 max-w-xs mx-auto">
          {sourceMessages.slice(-3).map((msg, i, arr) => (
            <p
              key={`${msg}-${i}`}
              className={`text-xs transition-opacity ${
                i === arr.length - 1 ? 'text-text-secondary' : 'text-text-muted'
              }`}
            >
              {msg}
            </p>
          ))}
        </div>
      )}

      {/* Narration feed (fetching + analyzing phases) */}
      {(phase === 'fetching' || phase === 'analyzing') && narrationEvents && narrationEvents.length > 0 && (
        <NarrationFeed events={narrationEvents} />
      )}

      {/* Analyzing phase — keyword-only note */}
      {phase === 'analyzing' && embeddingMode === 'keyword-only' && (
        <p className="text-xs text-text-muted mt-4">
          {t('firstRun.keywordMatching')}
        </p>
      )}
    </div>
  );
}
