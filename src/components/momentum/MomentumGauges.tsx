import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { CompoundAdvantageScore } from '../../types/autophagy';
import type { KnowledgeGap } from '../../types/innovation';
import type { RadarEntry } from '../tech-radar/RadarSVG';

// ---------------------------------------------------------------------------
// Sparkline
// ---------------------------------------------------------------------------

function Sparkline({ data }: { data: number[] }) {
  if (data.length < 2) return null;
  const max = Math.max(...data, 1);
  const min = Math.min(...data, 0);
  const range = max - min || 1;
  const w = 56, h = 18;
  const pts = data.map((v, i) =>
    `${(i / (data.length - 1)) * w},${h - ((v - min) / range) * h}`,
  ).join(' ');
  return (
    <svg width={w} height={h} className="inline-block">
      <polyline points={pts} fill="none" stroke="currentColor" strokeWidth="1.5" className="text-accent-gold" />
    </svg>
  );
}

function TrendArrow({ trend }: { trend: number }) {
  if (trend > 0.05) return <span className="text-green-400">{'\u2191'}</span>;
  if (trend < -0.05) return <span className="text-red-400">{'\u2193'}</span>;
  return <span className="text-text-muted">{'\u2192'}</span>;
}

// ---------------------------------------------------------------------------
// Single Gauge
// ---------------------------------------------------------------------------

function Gauge({ value, label, unit, color }: {
  value: string;
  label: string;
  unit?: string;
  color: string;
}) {
  return (
    <div className="flex flex-col items-center gap-0.5">
      <div className="flex items-baseline gap-0.5">
        <span className={`text-lg font-bold tabular-nums ${color}`}>{value}</span>
        {unit !== undefined && <span className="text-[10px] text-text-muted">{unit}</span>}
      </div>
      <span className="text-[9px] text-text-muted uppercase tracking-wider">{label}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MomentumGaugesProps {
  advantage: CompoundAdvantageScore | null;
  history: number[];
  entries: RadarEntry[];
  gaps: KnowledgeGap[];
}

export const MomentumGauges = memo(function MomentumGauges({
  advantage,
  history,
  entries,
  gaps,
}: MomentumGaugesProps) {
  const { t } = useTranslation();

  if (advantage === null) return null;

  const score = Math.round(advantage.score);
  const scoreColor = score >= 60 ? 'text-green-400' : score >= 30 ? 'text-accent-gold' : 'text-text-secondary';

  // Coverage: what % of your stack has NO critical/high gaps
  const critGaps = gaps.filter(g => g.gap_severity === 'critical' || g.gap_severity === 'high').length;
  const totalStack = entries.length;
  const coverage = totalStack > 0 ? Math.round(((totalStack - critGaps) / totalStack) * 100) : 100;
  const coverageColor = coverage >= 80 ? 'text-green-400' : coverage >= 50 ? 'text-accent-gold' : 'text-red-400';

  // Response rate: acted / (acted + expired)
  const acted = Number(advantage.windows_acted);
  const expired = Number(advantage.windows_expired);
  const total = acted + expired;
  const response = total > 0 ? Math.round((acted / total) * 100) : 100;
  const responseColor = response >= 80 ? 'text-green-400' : response >= 50 ? 'text-accent-gold' : 'text-red-400';

  // Lead time
  const leadHours = Math.round(advantage.avg_lead_time_hours);

  const sparkData = history.length >= 2
    ? history
    : [Math.max(advantage.score - 8, 0), advantage.score - 3, advantage.score];

  return (
    <div className="flex items-center justify-between px-6 py-3 bg-bg-secondary rounded-lg border border-border">
      {/* Advantage — primary gauge */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1">
          <span className={`text-2xl font-bold tabular-nums ${scoreColor}`}>{score}</span>
          <TrendArrow trend={advantage.trend} />
        </div>
        <div className="flex flex-col gap-0.5">
          <span className="text-[9px] text-text-muted uppercase tracking-wider">
            {t('momentum.compoundAdvantage')}
          </span>
          <Sparkline data={sparkData} />
        </div>
      </div>

      {/* Divider */}
      <div className="h-8 w-px bg-border" />

      {/* Secondary gauges */}
      <Gauge value={`${coverage}`} unit="%" label={t('momentum.gauge.coverage')} color={coverageColor} />
      <div className="h-8 w-px bg-border" />
      <Gauge value={`${response}`} unit="%" label={t('momentum.gauge.response')} color={responseColor} />
      <div className="h-8 w-px bg-border" />
      <Gauge value={leadHours > 0 ? `${leadHours}` : '--'} unit={leadHours > 0 ? 'h' : undefined} label={t('momentum.gauge.leadTime')} color="text-text-secondary" />
    </div>
  );
});
