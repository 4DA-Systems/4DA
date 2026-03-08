import { useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

function TrendArrow({ trend }: { trend: number }) {
  if (trend > 0.1) return <span className="text-green-400 text-xs">&#8593;</span>;
  if (trend < -0.1) return <span className="text-red-400 text-xs">&#8595;</span>;
  return <span className="text-text-muted text-xs">&#8594;</span>;
}

function MiniSparkline({ data }: { data: number[] }) {
  if (data.length < 2) return null;
  const max = Math.max(...data, 1);
  const min = Math.min(...data, 0);
  const range = max - min || 1;
  const width = 60;
  const height = 16;
  const points = data.map((v, i) => {
    const x = (i / (data.length - 1)) * width;
    const y = height - ((v - min) / range) * height;
    return `${x},${y}`;
  }).join(' ');

  return (
    <svg width={width} height={height} className="inline-block">
      <polyline
        points={points}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        className="text-[#D4AF37]"
      />
    </svg>
  );
}

export const CompoundAdvantageScore = memo(function CompoundAdvantageScore() {
  const { t } = useTranslation();
  const advantage = useAppStore(s => s.compoundAdvantage);
  const loadAdvantage = useAppStore(s => s.loadCompoundAdvantage);

  useEffect(() => {
    loadAdvantage();
  }, [loadAdvantage]);

  if (!advantage) return null;

  const displayScore = Math.round(advantage.score);
  const scoreColor = displayScore >= 60 ? 'text-green-400'
    : displayScore >= 30 ? 'text-[#D4AF37]'
    : 'text-text-secondary';

  return (
    <div className="flex items-center gap-4 px-4 py-2.5 bg-bg-secondary rounded-lg border border-border">
      <div className="flex items-center gap-2">
        <span className={`text-lg font-semibold tabular-nums ${scoreColor}`}>{displayScore}</span>
        <TrendArrow trend={advantage.trend} />
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-[10px] text-text-muted uppercase tracking-wider">{t('advantage.score')}</div>
        <div className="flex items-center gap-3 mt-0.5">
          <span className="text-[10px] text-text-secondary">
            {t('advantage.acted', { acted: advantage.windows_acted, opened: advantage.windows_opened })}
          </span>
          {advantage.avg_lead_time_hours > 0 && (
            <span className="text-[10px] text-text-secondary">
              {t('advantage.avgLead', { hours: Math.round(advantage.avg_lead_time_hours) })}
            </span>
          )}
        </div>
      </div>
      <MiniSparkline data={[
        Math.max(advantage.score - 10, 0),
        advantage.score - 5,
        advantage.score - 2,
        advantage.score,
      ]} />
    </div>
  );
});
