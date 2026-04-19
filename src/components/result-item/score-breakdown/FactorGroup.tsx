// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { FactorBar } from './FactorBar';
import type { Factor } from './factor-utils';

interface FactorGroupProps {
  label: string;
  factors: Factor[];
  comparisons: Factor[] | null;
  itemId: number;
  onFeedbackGiven: (factorKey: string, vote: 'up' | 'down') => void;
}

export function FactorGroup({
  label,
  factors,
  comparisons,
  itemId,
  onFeedbackGiven,
}: FactorGroupProps) {
  return (
    <div>
      <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">{label}</p>
      <div className="space-y-1.5">
        {factors.map(f => (
          <FactorBar
            key={f.key}
            factor={f}
            compareValue={comparisons?.find(c => c.key === f.key)?.value}
            itemId={itemId}
            onFeedbackGiven={onFeedbackGiven}
          />
        ))}
      </div>
    </div>
  );
}
