import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

interface ModuleHealth {
  module_id: string;
  module_name: string;
  score: number;
}

interface StreetHealthScore {
  overall: number;
  module_scores: ModuleHealth[];
  trend: string;
  top_action: string;
}

const TREND_ICONS: Record<string, { symbol: string; color: string }> = {
  improving: { symbol: '\u25B2', color: '#22C55E' },
  stable: { symbol: '\u25CF', color: '#A0A0A0' },
  declining: { symbol: '\u25BC', color: '#EF4444' },
};

export function StreetHealthBadge() {
  const { t } = useTranslation();
  const [streetHealth, setStreetHealth] = useState<StreetHealthScore | null>(null);

  useEffect(() => {
    invoke<StreetHealthScore>('get_street_health')
      .then(setStreetHealth)
      .catch((e) => console.warn('StreetHealthBadge: failed to load progress', e));
  }, []);

  if (!streetHealth) return null;

  const pct = Math.round(streetHealth.overall * 100);
  const trendInfo = TREND_ICONS[streetHealth.trend] || TREND_ICONS.stable;
  const scoreColor =
    pct >= 70 ? '#22C55E' : pct >= 40 ? '#D4AF37' : '#EF4444';

  return (
    <div
      className="rounded-xl p-4 mb-4"
      style={{ background: '#0A0A0A', border: '1px solid #2A2A2A' }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div
            className="w-12 h-12 rounded-full flex items-center justify-center text-lg font-bold"
            style={{
              background: `${scoreColor}15`,
              color: scoreColor,
              border: `2px solid ${scoreColor}40`,
            }}
          >
            {pct}
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold text-white">
                {t('playbook.health.title')}
              </span>
              <span className="text-xs" style={{ color: trendInfo.color }}>
                {trendInfo.symbol} {streetHealth.trend}
              </span>
            </div>
            <p className="text-xs mt-0.5" style={{ color: '#666666' }}>
              {streetHealth.top_action}
            </p>
          </div>
        </div>

        {/* Mini module score bar */}
        <div className="flex gap-0.5">
          {streetHealth.module_scores.map((m) => (
            <div
              key={m.module_id}
              className="w-2 rounded-full"
              style={{
                height: `${Math.max(8, m.score * 32)}px`,
                background:
                  m.score >= 0.7
                    ? '#22C55E'
                    : m.score >= 0.4
                      ? '#D4AF37'
                      : '#EF4444',
                opacity: 0.6 + m.score * 0.4,
              }}
              title={`${m.module_name}: ${Math.round(m.score * 100)}%`}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
