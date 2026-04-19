// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';

interface EngagementData {
  today_interactions: number;
  streak_days: number;
  heatmap: Array<{ date: string; day: string; count: number }>;
  accuracy_trend: 'improving' | 'declining' | 'stable';
  recent_positive_rate: string;
}

export const EngagementPulse = memo(function EngagementPulse() {
  const { t } = useTranslation();
  const [data, setData] = useState<EngagementData | null>(null);

  useEffect(() => {
    cmd('get_engagement_summary')
      .then(r => r as unknown as EngagementData)
      .then(setData)
      .catch((e: unknown) => console.warn('EngagementPulse: failed to load pulse data', e));
  }, []);

  if (!data) return null;

  const maxCount = Math.max(...data.heatmap.map((d) => d.count), 1);

  const trendIcon =
    data.accuracy_trend === 'improving'
      ? '\u2191'
      : data.accuracy_trend === 'declining'
        ? '\u2193'
        : '\u2192';

  const trendColor =
    data.accuracy_trend === 'improving'
      ? 'text-green-400'
      : data.accuracy_trend === 'declining'
        ? 'text-red-400'
        : 'text-text-secondary';

  return (
    <div className="flex items-center gap-4 px-4 py-2.5 bg-bg-secondary rounded-lg border border-border">
      {/* 7-day heatmap */}
      <div className="flex items-end gap-1" title={t('engagement.activity')}>
        {data.heatmap.map((day) => {
          const intensity = day.count === 0 ? 0 : Math.max(0.2, day.count / maxCount);
          return (
            <div key={day.date} className="flex flex-col items-center gap-0.5">
              <div
                className="w-3 rounded-sm transition-all"
                style={{
                  height: `${Math.max(4, intensity * 20)}px`,
                  backgroundColor: day.count === 0 ? '#2A2A2A' : `rgba(34, 197, 94, ${intensity})`,
                }}
                title={`${day.day}: ${day.count} interactions`}
              />
              <span className="text-[8px] text-text-muted">{day.day.charAt(0)}</span>
            </div>
          );
        })}
      </div>

      {/* Streak */}
      <div className="flex items-center gap-1.5" title={`${data.streak_days} day streak`}>
        <span className="text-xs font-mono font-medium text-orange-400">
          {data.streak_days}d
        </span>
        <span className="text-[10px] text-text-muted">{t('engagement.streak')}</span>
      </div>

      {/* Trend */}
      <div className="flex items-center gap-1" title={`Learning trend: ${data.accuracy_trend} (${data.recent_positive_rate} positive)`}>
        <span className={`text-sm font-medium ${trendColor}`}>{trendIcon}</span>
        <span className="text-[10px] text-text-muted">{data.recent_positive_rate}</span>
      </div>
    </div>
  );
});
