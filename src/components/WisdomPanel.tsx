// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, memo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';

const FeedbackButtons = memo(function FeedbackButtons({ query }: { query: string }) {
  const { t } = useTranslation();
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const handleFeedback = useCallback(async (outcome: string) => {
    setSubmitting(true);
    try {
      // Use the query as decision reference since we don't have decision_id from transmute
      await cmd('run_awe_feedback', {
        decisionId: `inline_${Date.now()}`,
        outcome,
        details: query,
      });
      setSubmitted(true);
    } catch {
      // Silently fail — feedback is best-effort
    } finally {
      setSubmitting(false);
    }
  }, [query]);

  if (submitted) {
    return (
      <p className="text-xs text-text-muted italic">{t('wisdom.outcomeRecorded')}</p>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <span className="text-xs text-text-muted">{t('wisdom.howDidItTurnOut')}</span>
      {(['confirmed', 'refuted', 'partial', 'too_early'] as const).map(outcome => (
        <button
          key={outcome}
          onClick={() => handleFeedback(outcome)}
          disabled={submitting}
          className="text-xs px-2 py-0.5 rounded border border-border/50 text-text-muted hover:text-text-secondary hover:border-border transition-colors disabled:opacity-30"
        >
          {t(`wisdom.outcome_${outcome}`)}
        </button>
      ))}
    </div>
  );
});

interface TransmuteResult {
  wisdom: string;
  confidence: number;
  watch_for: string[];
  mode: string;
}

/**
 * WisdomPanel — real decision analysis inline in Insights view.
 *
 * Calls AWE's transmutation pipeline via Tauri command.
 * Shows actual wisdom output — not a placeholder.
 * Three modes: Perspective (voice), Analysis (structured), Challenge.
 */
export const WisdomPanel = memo(function WisdomPanel() {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<TransmuteResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [mode, setMode] = useState<'voice' | 'challenge' | 'structured'>('voice');

  const handleTransmute = useCallback(async () => {
    const trimmed = query.trim();
    if (!trimmed || loading) return;

    setLoading(true);
    setResult(null);
    setError(null);

    try {
      const raw = await cmd('run_awe_transmute', { query: trimmed, mode });
      const parsed: TransmuteResult = JSON.parse(raw);
      setResult(parsed);
    } catch (e) {
      setError(typeof e === 'string' ? e : 'AWE analysis failed. Ensure the AWE binary is built.');
    } finally {
      setLoading(false);
    }
  }, [query, mode, loading]);

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">{t('wisdom.title')}</h3>
        <p className="text-xs text-text-muted mt-1">
          {t('wisdom.subtitle')}
        </p>
      </div>

      <div className="p-5 space-y-3">
        {/* Query input */}
        <div className="flex gap-2">
          <input
            type="text"
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleTransmute()}
            placeholder={t('wisdom.placeholder')}
            className="flex-1 bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-sm text-text-primary placeholder:text-text-muted/50 focus:outline-none focus:border-text-muted/50"
          />
          <button
            onClick={handleTransmute}
            disabled={!query.trim() || loading}
            className="px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-sm text-text-secondary hover:text-white hover:border-text-muted/50 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          >
            {loading ? t('wisdom.analyzing') : t('wisdom.analyze')}
          </button>
        </div>

        {/* Mode selector */}
        <div className="flex gap-2">
          {([
            { key: 'voice' as const, label: t('wisdom.perspective') },
            { key: 'structured' as const, label: t('wisdom.analysis') },
            { key: 'challenge' as const, label: t('wisdom.challenge') },
          ]).map(({ key, label }) => (
            <button
              key={key}
              onClick={() => setMode(key)}
              className={`text-xs px-2.5 py-1 rounded transition-colors ${
                mode === key
                  ? 'bg-bg-tertiary text-white border border-border'
                  : 'text-text-muted hover:text-text-secondary'
              }`}
            >
              {label}
            </button>
          ))}
        </div>

        {/* Loading state */}
        {loading && (
          <div className="bg-bg-primary rounded-lg border border-border/50 p-4">
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-accent-gold animate-pulse" />
              <span className="text-xs text-text-muted">
                {t('wisdom.pipelineRunning')}
              </span>
            </div>
          </div>
        )}

        {/* Error state */}
        {error && (
          <div className="bg-error/10 rounded-lg border border-error/20 p-4">
            <p className="text-xs text-error">{error}</p>
          </div>
        )}

        {/* Result */}
        {result && (
          <div className="bg-bg-primary rounded-lg border border-border/50 p-4 space-y-3">
            {/* Wisdom text */}
            <div className="text-sm text-text-primary leading-relaxed whitespace-pre-wrap">
              {result.wisdom}
            </div>

            {/* Confidence */}
            <div className="flex items-center gap-3 text-xs text-text-muted">
              <span>
                {t('wisdom.confidence', { value: (result.confidence * 100).toFixed(0) })}
              </span>
              <span className="text-text-muted/40">|</span>
              <span>
                {result.mode === 'voice' ? t('wisdom.perspective') : result.mode === 'challenge' ? t('wisdom.challenge') : t('wisdom.analysis')}
              </span>
            </div>

            {/* Watch for */}
            {result.watch_for.length > 0 && (
              <div className="pt-2 border-t border-border/30">
                <p className="text-xs text-text-muted mb-1">{t('wisdom.watchFor')}</p>
                {result.watch_for.map((w, i) => (
                  <p key={i} className="text-xs text-text-secondary ms-2">
                    {w}
                  </p>
                ))}
              </div>
            )}

            {/* Feedback — record decision outcomes */}
            <FeedbackButtons query={query} />
          </div>
        )}
      </div>
    </div>
  );
});
