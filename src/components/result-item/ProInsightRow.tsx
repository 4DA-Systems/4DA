import { useState } from 'react';
import type { SourceRelevance } from '../../types';
import { useLicense } from '../../hooks/use-license';
import { useAppStore } from '../../store';
import { formatScore } from '../../utils/score';

interface ProInsightRowProps {
  item: SourceRelevance;
}

const SIGNAL_LABELS: Record<string, string> = {
  security_alert: 'Security Alert',
  breaking_change: 'Breaking Change',
  tool_discovery: 'Tool Discovery',
  tech_trend: 'Emerging Trend',
  learning: 'Learning Resource',
  competitive_intel: 'Competitive Intel',
};

/**
 * Inline Pro intelligence row shown on collapsed feed items.
 *
 * Free users: see signal badges + contextual upgrade path:
 *   - Trial not started → "Start trial to see why"
 *   - Trial expired → "See why" links to 4da.ai/streets
 * Pro users: see relevance breakdown, signal chain context, knowledge gap alerts.
 */
export function ProInsightRow({ item }: ProInsightRowProps) {
  const { isPro, trialStatus } = useLicense();
  const b = item.score_breakdown;

  // Only show on items that have something interesting to surface
  const hasSignal = !!item.signal_type;
  const hasDepMatch = b?.matched_deps && b.matched_deps.length > 0;
  const hasHighScore = item.top_score >= 0.5;

  if (!hasSignal && !hasDepMatch && !hasHighScore) return null;

  if (isPro) {
    return <ProInsightDetail item={item} />;
  }

  const canStartTrial = !trialStatus?.started_at;

  // Free user: show teaser with contextual upgrade path
  return (
    <div className="mt-1 pl-[4.25rem] flex items-center gap-2">
      {hasSignal && (
        <span className="text-[10px] text-cyan-400/70">
          {SIGNAL_LABELS[item.signal_type!] || item.signal_type}
        </span>
      )}
      {hasDepMatch && (
        <span className="text-[10px] text-emerald-400/70">
          Affects {b!.matched_deps!.slice(0, 2).join(', ')}
        </span>
      )}
      {canStartTrial ? (
        <InlineTrialStart score={item.top_score} />
      ) : (
        <a
          href="https://4da.ai/streets"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-0.5 text-[10px] text-[#D4AF37]/50 hover:text-[#D4AF37] transition-colors ml-auto"
        >
          <ProStar />
          See why {formatScore(item.top_score)}
        </a>
      )}
    </div>
  );
}

/** Inline trial start button — no external redirect */
function InlineTrialStart({ score }: { score: number }) {
  const startTrial = useAppStore((s) => s.startTrial);
  const [starting, setStarting] = useState(false);

  const handleStart = async (e: React.MouseEvent) => {
    e.stopPropagation();
    setStarting(true);
    await startTrial();
    setStarting(false);
  };

  return (
    <button
      onClick={handleStart}
      disabled={starting}
      className="inline-flex items-center gap-0.5 text-[10px] text-[#D4AF37]/50 hover:text-[#D4AF37] transition-colors ml-auto disabled:opacity-50"
    >
      <ProStar />
      {starting ? 'Starting...' : `Try Pro free — see why ${formatScore(score)}`}
    </button>
  );
}

function ProStar() {
  return (
    <svg width="8" height="8" viewBox="0 0 16 16" fill="none">
      <path d="M8 1L10 6H15L11 9.5L12.5 15L8 11.5L3.5 15L5 9.5L1 6H6L8 1Z" fill="currentColor"/>
    </svg>
  );
}

/** Full inline intelligence for Pro users */
function ProInsightDetail({ item }: { item: SourceRelevance }) {
  const b = item.score_breakdown;
  const parts: string[] = [];

  // Score breakdown explanation
  if (b) {
    if (b.context_score > 0.3) parts.push('Strong context match');
    if (b.dep_match_score && b.dep_match_score > 0.1)
      parts.push(`Stack: ${b.matched_deps?.slice(0, 3).join(', ')}`);
    if (b.interest_score > 0.3) parts.push('Interest match');
    if (b.ace_boost > 0.1) parts.push('Active in recent work');
    if (b.affinity_mult > 1.2) parts.push('Learned preference');
    if (b.llm_reason) parts.push(b.llm_reason);
  }

  if (parts.length === 0 && !item.signal_action) return null;

  return (
    <div className="mt-1 pl-[4.25rem] space-y-0.5">
      {/* Signal action (tactical advice) */}
      {item.signal_action && (
        <div className="text-[10px] text-cyan-400/80 leading-snug">
          {item.signal_action}
        </div>
      )}
      {/* Score explanation */}
      {parts.length > 0 && (
        <div className="text-[10px] text-gray-500 leading-snug">
          {parts.slice(0, 3).join(' · ')}
        </div>
      )}
    </div>
  );
}
