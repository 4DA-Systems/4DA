import { useState, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface AweSummary {
  available: boolean;
  decisions: number;
  principles: number;
  pending: number;
  top_principle: string | null;
  health: string | null;
}

interface AweRecall {
  principles?: string[];
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** Gold-tinted stat cell with label and value */
function StatCell({ value, label }: { value: string | number; label: string }) {
  return (
    <div className="text-center">
      <div className="text-lg font-semibold text-white tabular-nums">{value}</div>
      <div className="text-[10px] text-text-muted uppercase tracking-wider">{label}</div>
    </div>
  );
}

/** Confidence/health bar */
function HealthBar({ health }: { health: string | null }) {
  // Parse health string — AWE returns things like "healthy", "learning", "needs_feedback"
  const level = health ?? 'learning';
  const config: Record<string, { width: string; color: string; label: string }> = {
    healthy:        { width: '85%', color: 'bg-green-500',  label: 'Healthy' },
    good:           { width: '70%', color: 'bg-green-500',  label: 'Good' },
    learning:       { width: '40%', color: 'bg-accent-gold', label: 'Learning' },
    needs_feedback: { width: '25%', color: 'bg-amber-500',  label: 'Needs Feedback' },
    cold:           { width: '10%', color: 'bg-text-muted',  label: 'Cold Start' },
  };
  const c = config[level] ?? config.learning!;

  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-1.5 bg-bg-primary rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all ${c.color}`}
          style={{ width: c.width }}
        />
      </div>
      <span className="text-[10px] text-text-muted flex-shrink-0">{c.label}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

/**
 * AWE Wisdom Card — surfaces the Artificial Wisdom Engine's accumulated
 * knowledge in a professional, intuitive card.
 *
 * Shows: wisdom graph stats (decisions/principles/pending), validated principles,
 * health indicator, and pending feedback nudge.
 */
export const AweWisdomCard = memo(function AweWisdomCard() {
  const { t } = useTranslation();
  const [summary, setSummary] = useState<AweSummary | null>(null);
  const [principles, setPrinciples] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [expanded, setExpanded] = useState(false);

  const loadData = useCallback(async () => {
    try {
      const [summaryRaw, recallRaw] = await Promise.allSettled([
        cmd('get_awe_summary'),
        cmd('run_awe_recall', { domain: 'software-engineering' }),
      ]);

      if (summaryRaw.status === 'fulfilled') {
        try {
          const parsed: AweSummary = JSON.parse(summaryRaw.value);
          setSummary(parsed);
        } catch { /* non-JSON response */ }
      }

      if (recallRaw.status === 'fulfilled') {
        try {
          const parsed: AweRecall = JSON.parse(recallRaw.value);
          if (parsed.principles && parsed.principles.length > 0) {
            setPrinciples(parsed.principles);
          }
        } catch { /* non-JSON response */ }
      }
    } catch {
      // AWE unavailable — card will show unavailable state
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadData(); }, [loadData]);

  // Don't render if AWE is unavailable or still loading
  if (loading) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-4">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-accent-gold/50 animate-pulse" />
          <span className="text-xs text-text-muted">{t('awe.loading')}</span>
        </div>
      </div>
    );
  }

  if (!summary || !summary.available) {
    return null; // AWE not installed — don't show card
  }

  const hasPrinciples = principles.length > 0;
  const displayPrinciples = expanded ? principles : principles.slice(0, 3);
  const hasPending = summary.pending > 0;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-4 py-3 border-b border-border/50 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-accent-gold text-sm">{'\u25C7'}</span>
          <h4 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium">
            {t('awe.cardTitle')}
          </h4>
        </div>
        {hasPending && (
          <span className="text-[9px] px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-400 font-medium">
            {t('awe.pendingCount', { count: summary.pending })}
          </span>
        )}
      </div>

      <div className="p-4 space-y-4">
        {/* Stats row */}
        <div className="grid grid-cols-3 gap-2">
          <StatCell value={summary.decisions} label={t('awe.decisions')} />
          <StatCell value={summary.principles} label={t('awe.principles')} />
          <StatCell
            value={summary.decisions > 0 ? `${Math.round(((summary.decisions - summary.pending) / summary.decisions) * 100)}%` : '--'}
            label={t('awe.feedbackRate')}
          />
        </div>

        {/* Health bar */}
        <HealthBar health={summary.health} />

        {/* Validated Principles */}
        {hasPrinciples && (
          <div className="pt-2 border-t border-border/30">
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-2">
              {t('awe.validatedPrinciples')}
            </h5>
            <ul className="space-y-2">
              {displayPrinciples.map((p, i) => (
                <li key={i} className="text-xs text-text-secondary leading-relaxed flex items-start gap-2">
                  <span className="text-accent-gold/60 mt-0.5 flex-shrink-0">{'\u25C6'}</span>
                  <span>{p}</span>
                </li>
              ))}
            </ul>
            {principles.length > 3 && (
              <button
                onClick={() => setExpanded(!expanded)}
                className="text-[10px] text-text-muted hover:text-text-secondary mt-2 transition-colors"
              >
                {expanded
                  ? t('awe.showLess')
                  : t('awe.showMore', { count: principles.length - 3 })}
              </button>
            )}
          </div>
        )}

        {/* Top Principle highlight (when no full list but summary has one) */}
        {!hasPrinciples && summary.top_principle && (
          <div className="pt-2 border-t border-border/30">
            <div className="flex items-start gap-2">
              <span className="text-accent-gold/60 mt-0.5 flex-shrink-0">{'\u25C6'}</span>
              <p className="text-xs text-text-secondary leading-relaxed italic">
                {summary.top_principle}
              </p>
            </div>
          </div>
        )}

        {/* Empty state — AWE needs more decisions */}
        {!hasPrinciples && !summary.top_principle && summary.decisions < 5 && (
          <div className="pt-2 border-t border-border/30">
            <p className="text-xs text-text-muted">
              {t('awe.earlyStage')}
            </p>
          </div>
        )}

        {/* Pending feedback nudge */}
        {hasPending && (
          <div className="flex items-center gap-2 pt-2 border-t border-border/30">
            <div className="w-1.5 h-1.5 rounded-full bg-amber-400 flex-shrink-0" />
            <p className="text-[10px] text-amber-400/80">
              {t('awe.pendingNudge', { count: summary.pending })}
            </p>
          </div>
        )}
      </div>
    </div>
  );
});
