import { memo, useState, useEffect } from 'react';
import { cmd } from '../lib/commands';

export const AccuracySparkline = memo(function AccuracySparkline() {
  const [points, setPoints] = useState<number[]>([]);

  useEffect(() => {
    cmd('get_intelligence_growth')
      .then((data) => {
        const last8 = data.snapshots.slice(-8).map((s) => s.accuracy);
        if (last8.length >= 2) setPoints(last8);
      })
      .catch(() => {});
  }, []);

  if (points.length < 2) return null;

  const w = 40;
  const h = 16;
  const min = Math.min(...points);
  const max = Math.max(...points);
  const range = max - min || 1;
  const trending = points[points.length - 1] > points[0];
  const color = trending ? '#22C55E' : '#A0A0A0';

  const d = points
    .map((v, i) => {
      const x = (i / (points.length - 1)) * w;
      const y = h - ((v - min) / range) * (h - 2) - 1;
      return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)} ${y.toFixed(1)}`;
    })
    .join(' ');

  return (
    <svg
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      className="inline-block align-middle"
      aria-label={`Accuracy trend: ${trending ? 'improving' : 'stable'}`}
    >
      <path d={d} fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
});
