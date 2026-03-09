import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { ProGate } from './ProGate';

interface TopicAffinity {
  topic: string;
  positive_signals: number;
  negative_signals: number;
  total_exposures: number;
  affinity_score: number;
  confidence: number;
  last_interaction: string;
}

interface AffinityDelta {
  topic: string;
  score: number;
  positives: number;
  negatives: number;
  direction: 'up' | 'down' | 'stable';
}

export const ScoringDelta = memo(function ScoringDelta() {
  const { t } = useTranslation();
  const [deltas, setDeltas] = useState<AffinityDelta[]>([]);

  useEffect(() => {
    const load = async () => {
      try {
        const result = await cmd('ace_get_topic_affinities');
        const affinities = result.affinities as unknown as TopicAffinity[];
        if (!affinities || affinities.length === 0) return;

        // Filter to topics with recent interactions and meaningful signal
        const meaningful = affinities
          .filter(a => (a.positive_signals + a.negative_signals) >= 2 && a.confidence > 0.1)
          .map(a => ({
            topic: a.topic,
            score: a.affinity_score,
            positives: a.positive_signals,
            negatives: a.negative_signals,
            direction: (a.affinity_score > 0.15 ? 'up' : a.affinity_score < -0.15 ? 'down' : 'stable') as AffinityDelta['direction'],
          }))
          .filter(d => d.direction !== 'stable')
          .sort((a, b) => Math.abs(b.score) - Math.abs(a.score))
          .slice(0, 6);

        setDeltas(meaningful);
      } catch {
        // Scoring delta is supplementary
      }
    };
    load();
  }, []);

  if (deltas.length === 0) return null;

  const gaining = deltas.filter(d => d.direction === 'up');
  const losing = deltas.filter(d => d.direction === 'down');

  return (
    <ProGate feature={t('scoringDelta.feature', 'Scoring Delta')}>
      <div className="mb-4 bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-4 py-3 flex items-center gap-3 border-b border-border/50">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center flex-shrink-0">
            <span className="text-sm text-text-secondary">&#x2194;</span>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">
              {t('scoringDelta.title', 'Your scoring has shifted')}
            </h3>
            <p className="text-xs text-text-muted">
              {t('scoringDelta.subtitle', 'Based on your feedback')}
            </p>
          </div>
        </div>

        <div className="px-4 py-3 space-y-3">
          {/* Topics gaining weight */}
          {gaining.length > 0 && (
            <div>
              <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
                {t('scoringDelta.gaining', 'Gaining weight')}
              </p>
              <div className="space-y-1">
                {gaining.map(d => (
                  <div key={d.topic} className="flex items-center gap-2">
                    <span className="text-green-400 text-xs">{'\u2191'}</span>
                    <span className="text-xs text-text-secondary flex-1 truncate">{d.topic}</span>
                    <span className="text-[10px] font-mono text-green-400">
                      +{Math.round(d.score * 100)}%
                    </span>
                    <span className="text-[10px] text-text-muted">
                      {d.positives} saves
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Topics losing weight */}
          {losing.length > 0 && (
            <div>
              <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
                {t('scoringDelta.losing', 'Losing weight')}
              </p>
              <div className="space-y-1">
                {losing.map(d => (
                  <div key={d.topic} className="flex items-center gap-2">
                    <span className="text-amber-400 text-xs">{'\u2193'}</span>
                    <span className="text-xs text-text-secondary flex-1 truncate">{d.topic}</span>
                    <span className="text-[10px] font-mono text-amber-400">
                      {Math.round(d.score * 100)}%
                    </span>
                    <span className="text-[10px] text-text-muted">
                      {d.negatives} dismissed
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </ProGate>
  );
});
