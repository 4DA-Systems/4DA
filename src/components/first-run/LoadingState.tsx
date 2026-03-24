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
        <div className="mb-5 px-4 py-3 bg-bg-secondary rounded-lg border border-orange-500/10 text-start">
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
  estimatedSeconds?: number;
  onSkipAhead?: () => void;
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
  estimatedSeconds: estimatedSecondsProp,
  onSkipAhead,
}: LoadingStateProps) {
  const { t } = useTranslation();

  useEffect(() => { registerGameComponent('game-simplex-unfold'); }, []);

  // Estimated time remaining — initialized from parent's source-count estimate
  // Uses a ref-based counter to avoid infinite timer chains in test environments
  const initialSeconds = estimatedSecondsProp ?? 240;
  const [estimatedSeconds, setEstimatedSeconds] = useState(initialSeconds);
  const counterRef = useRef(initialSeconds);

  // Show "Skip ahead" button after 5 seconds in fetching/analyzing phases
  const [showSkip, setShowSkip] = useState(false);
  useEffect(() => {
    if (phase !== 'fetching' && phase !== 'analyzing') return;
    const timer = setTimeout(() => setShowSkip(true), 5000);
    return () => clearTimeout(timer);
  }, [phase]);

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
  if (phase === 'intelligence') {
    if (scanSummary) {
      return <IntelligencePreview summary={scanSummary} />;
    }
    return (
      <div className="text-center px-8 max-w-md animate-fade-in">
        <div className="mb-6">
          <VoidEngine size={80} />
        </div>
        <h2 className="text-xl font-medium text-white mb-2">
          {t('firstRun.discoveringStack', 'Discovering your stack...')}
        </h2>
        <p className="text-xs text-text-muted animate-pulse">
          {t('firstRun.startingAnalysis', 'Starting content analysis...')}
        </p>
      </div>
    );
  }

  return (
    <div className="text-center px-8 max-w-md">
      <div className="mb-6">
        <VoidEngine size={80} />
      </div>

      {phase === 'preparing' && (
        <div className="w-[120px] h-[120px] mx-auto mb-3 opacity-80">
          <game-simplex-unfold style={{ width: '120px', height: '120px', display: 'block' }} />
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
        <p className={`text-sm text-text-secondary mb-4${estimatedSeconds <= 0 ? ' animate-pulse' : ''}`} aria-live="polite">
          {estimatedSeconds <= 0
            ? t('firstRun.estimatedTimeFinishing', 'Finishing up...')
            : estimatedSeconds < 30
              ? t('firstRun.estimatedTimeAlmost', 'Almost there...')
              : estimatedSeconds < 60
                ? t('firstRun.estimatedTimeLessThanMinute', '< 1 minute remaining')
                : estimatedSeconds <= 120
                  ? t('firstRun.estimatedTimeOneToTwo', '~1-2 minutes remaining')
                  : t('firstRun.estimatedTimeMinutes', { minutes: Math.ceil(estimatedSeconds / 60), defaultValue: '~{{minutes}} minutes remaining' })
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

      {/* Skip ahead button — appears after 5 seconds in fetching/analyzing only */}
      {showSkip && onSkipAhead && (
        <button
          onClick={onSkipAhead}
          className="mt-6 text-xs text-text-muted hover:text-white transition-colors"
        >
          {t('firstRun.skipAhead', 'Skip ahead')} &rarr;
        </button>
      )}
    </div>
  );
}
