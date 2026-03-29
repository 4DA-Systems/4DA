import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface WorkTopic {
  topic: string;
  weight: number;
  confidence: number;
  last_seen: string;
}

export interface ActiveWorkData {
  topics: WorkTopic[];
  file_changes_last_hour: number;
  active_project: string | null;
}

export interface ActiveWorkSectionProps {
  data: ActiveWorkData | null;
}

// ---------------------------------------------------------------------------
// Activity dots — visual intensity indicator
// ---------------------------------------------------------------------------

function ActivityDots({ weight }: { weight: number }) {
  const filled = Math.round(weight * 5);
  return (
    <div className="flex gap-0.5">
      {Array.from({ length: 5 }, (_, i) => (
        <span
          key={i}
          className={`w-1 h-1 rounded-full ${i < filled ? 'bg-green-400' : 'bg-[#2A2A2A]'}`}
        />
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const ActiveWorkSection = memo(function ActiveWorkSection({ data }: ActiveWorkSectionProps) {
  const { t } = useTranslation();

  const visibleTopics = useMemo(() => {
    if (data === null || data.topics.length === 0) return [];
    return data.topics.slice(0, 10);
  }, [data]);

  // Don't render if no active work detected
  if (data === null || (visibleTopics.length === 0 && data.file_changes_last_hour === 0)) {
    return null;
  }

  return (
    <section aria-label={t('momentum.activeWork')}>
      <div className="flex items-center justify-between mb-3 px-1">
        <h3 className="text-[10px] text-text-muted uppercase tracking-wider font-medium">
          {t('momentum.activeWork')}
        </h3>
        {data.active_project !== null && data.active_project !== '' && (
          <span className="text-[10px] text-text-muted font-mono">{data.active_project}</span>
        )}
      </div>

      <div className="bg-bg-secondary rounded-lg border border-border p-4">
        {/* Topic pills */}
        {visibleTopics.length > 0 && (
          <div className="flex flex-wrap gap-2 mb-3">
            {visibleTopics.map(topic => (
              <div
                key={topic.topic}
                className="inline-flex items-center gap-2 px-2.5 py-1 rounded-md bg-[#0F0F0F] border border-border/50 text-xs text-white"
              >
                <span className="truncate max-w-[120px]">{topic.topic}</span>
                <ActivityDots weight={topic.weight} />
              </div>
            ))}
          </div>
        )}

        {/* Activity summary */}
        <div className="flex items-center gap-3 text-[10px] text-text-muted">
          {data.file_changes_last_hour > 0 && (
            <span>
              {t('momentum.filesChanged', { count: data.file_changes_last_hour })}
            </span>
          )}
          {visibleTopics.length > 0 && (
            <span>
              {t('momentum.topicsDetected', { count: visibleTopics.length })}
            </span>
          )}
        </div>
      </div>
    </section>
  );
});
