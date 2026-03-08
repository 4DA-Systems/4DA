import { useEffect, useState } from 'react';
import { useGameComponent } from '../../hooks/use-game-component';
import { getRelativeTime, getFreshnessColor } from '../../utils/briefing-parser';

/** GAME atmosphere background that responds to briefing signal data. */
export function BriefingAtmosphere({ signalCount, topCount, hasContent }: { signalCount: number; topCount: number; hasContent: boolean }) {
  const { containerRef, elementRef } = useGameComponent('game-briefing-atmosphere');

  useEffect(() => {
    elementRef.current?.setParam?.('quality', hasContent ? 0.7 : 0.2);
    elementRef.current?.setParam?.('signal_heat', Math.min(topCount / 20, 1));
    elementRef.current?.setParam?.('decision_pressure', Math.min(signalCount / 5, 1));
  }, [signalCount, topCount, hasContent, elementRef]);

  return <div ref={containerRef} className="w-full h-16 rounded-lg overflow-hidden opacity-50 -mb-2" aria-hidden="true" />;
}

/** Isolated tick timer — only re-renders itself every 60s, not the whole view. */
export function RelativeTimestamp({ date }: { date: Date }) {
  const [, setTick] = useState(0);
  useEffect(() => {
    const interval = setInterval(() => setTick(t => t + 1), 60_000);
    return () => clearInterval(interval);
  }, []);
  return (
    <span className={`text-xs font-medium ${getFreshnessColor(date)}`}>
      {getRelativeTime(date)}
    </span>
  );
}

/** Stable skeleton widths — no Math.random() re-renders. */
export const SKELETON_WIDTHS = [85, 92, 78, 88, 70, 95];
