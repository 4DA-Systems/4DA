// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState } from 'react';
import { getRelativeTime, getFreshnessColor } from '../../utils/briefing-parser';

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
