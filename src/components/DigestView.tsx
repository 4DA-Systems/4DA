import { useState, useEffect } from 'react';
import { cmd } from '../lib/commands';
import { useTranslation } from 'react-i18next';

interface DigestData {
  generated_at: string;
  period_start: string;
  period_end: string;
  highlights: Array<{ title: string; url: string | null; score: number; source_type: string }>;
  top_topics: Array<{ topic: string; interactions: number; trend: string }>;
  stats: {
    total_items_analyzed: number;
    relevant_items: number;
    avg_relevance_score: number;
  };
}

export function DigestView() {
  const { t } = useTranslation();
  const [digest, setDigest] = useState<DigestData | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    cmd('get_latest_digest')
      .then(r => r as unknown as DigestData)
      .then(setDigest)
      .catch((e: unknown) => console.warn('DigestView: failed to load digest', e))
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <div className="animate-pulse h-20 bg-bg-tertiary rounded-lg" />;
  if (!digest) return null;

  const periodLabel = `${digest.period_start.slice(0, 10)} \u2014 ${digest.period_end.slice(0, 10)}`;

  return (
    <div className="bg-bg-secondary border border-border rounded-lg p-4 mb-4">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-medium text-white">
          {t('digest.title', 'Weekly Digest')}
        </h3>
        <span className="text-xs text-text-muted">{periodLabel}</span>
      </div>

      <div className="grid grid-cols-3 gap-3 mb-3">
        <div className="text-center">
          <div className="text-lg font-semibold text-white">{digest.stats.total_items_analyzed}</div>
          <div className="text-[10px] text-text-muted">{t('digest.analyzed', 'analyzed')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-semibold text-green-400">{digest.stats.relevant_items}</div>
          <div className="text-[10px] text-text-muted">{t('digest.relevant', 'relevant')}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-semibold text-accent-gold">
            {Math.round(digest.stats.avg_relevance_score * 100)}%
          </div>
          <div className="text-[10px] text-text-muted">{t('digest.avgScore', 'avg score')}</div>
        </div>
      </div>

      {digest.highlights.length > 0 && (
        <div className="space-y-1.5">
          <h4 className="text-xs text-text-muted font-medium">{t('digest.topItems', 'Top Items')}</h4>
          {digest.highlights.slice(0, 5).map((item, i) => (
            <div key={i} className="flex items-center gap-2 text-xs">
              <span className="text-text-muted w-4">{i + 1}.</span>
              <span className="text-white truncate flex-1">{item.title}</span>
              <span className="text-text-muted">{item.source_type}</span>
            </div>
          ))}
        </div>
      )}

      {digest.top_topics.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1">
          {digest.top_topics.slice(0, 6).map(topic => (
            <span key={topic.topic} className="px-1.5 py-0.5 bg-white/5 text-text-secondary text-[10px] rounded">
              {topic.topic} ({topic.interactions})
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
