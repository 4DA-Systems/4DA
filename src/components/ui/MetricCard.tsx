import { memo } from 'react';

interface MetricCardProps {
  label: string;
  value: string | number;
  delta?: number;
  deltaLabel?: string;
  className?: string;
}

export const MetricCard = memo(function MetricCard({
  label,
  value,
  delta,
  deltaLabel,
  className = '',
}: MetricCardProps) {
  const isPositive = delta && delta > 0;
  const isNegative = delta && delta < 0;

  return (
    <div className={`bg-bg-tertiary rounded-lg p-3 border border-border/50 ${className}`}>
      <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1">{label}</p>
      <div className="flex items-baseline gap-2">
        <span className="text-lg font-semibold text-white">{value}</span>
        {delta !== undefined && delta !== 0 && (
          <span
            className={`text-xs font-medium ${
              isPositive ? 'text-[#22C55E]' : isNegative ? 'text-[#EF4444]' : 'text-text-muted'
            }`}
          >
            {isPositive ? '+' : ''}{typeof delta === 'number' ? delta.toFixed(1) : delta}
            {deltaLabel ? ` ${deltaLabel}` : ''}
          </span>
        )}
      </div>
    </div>
  );
});
