// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, memo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

// ============================================================================
// Types
// ============================================================================

interface Snapshot {
  date: string;
  label: string;
}

export interface TemporalSliderProps {
  onSnapshotChange: (date: string | null) => void;
}

// ============================================================================
// Component
// ============================================================================

export const TemporalSlider = memo(function TemporalSlider({ onSnapshotChange }: TemporalSliderProps) {
  const { t } = useTranslation();
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    cmd('get_radar_snapshots')
      .then((data) => {
        const mapped = data.map((s) => ({
          date: s.date,
          label: new Date(s.date).toLocaleDateString(undefined, {
            month: 'short',
            day: 'numeric',
          }),
        }));
        setSnapshots(mapped);
        setSelectedIndex(mapped.length); // rightmost = "Current"
      })
      .catch(() => setSnapshots([]))
      .finally(() => setLoading(false));
  }, []);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const idx = parseInt(e.target.value, 10);
      setSelectedIndex(idx);
      if (idx >= snapshots.length) {
        onSnapshotChange(null);
      } else {
        onSnapshotChange(snapshots[idx]!.date);
      }
    },
    [snapshots, onSnapshotChange],
  );

  const hasSnapshots = snapshots.length > 0;
  const max = snapshots.length; // index = snapshots.length means "Current"
  const displayLabel =
    selectedIndex >= snapshots.length || selectedIndex < 0
      ? t('techRadar.current')
      : snapshots[selectedIndex]!.label;

  if (loading) return null;

  return (
    <div className="px-5 py-2 border-t border-border flex items-center gap-3">
      <span className="text-[10px] text-text-muted whitespace-nowrap">{t('techRadar.timeline')}</span>
      <input
        type="range"
        min={0}
        max={max}
        value={selectedIndex < 0 ? max : selectedIndex}
        onChange={handleChange}
        disabled={!hasSnapshots}
        aria-label={t('techRadar.timeline')}
        className="flex-1 h-1 appearance-none bg-border rounded-full cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed accent-accent-gold"
        style={{ colorScheme: 'dark' }}
      />
      <span className="text-[10px] text-text-secondary font-mono w-16 text-end whitespace-nowrap">
        {displayLabel}
      </span>
    </div>
  );
});
