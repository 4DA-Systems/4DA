interface SunSparklineProps {
  sunId: string;
}

export function SunSparkline({ sunId }: SunSparklineProps) {
  // Generate sparkline from last 7 run results stored in sun_runs
  // For now, use a placeholder based on sunId hash for visual consistency
  // The actual data will come from the sun's data field once we have history
  const points = generateSparklinePoints(sunId);

  const width = 48;
  const height = 16;
  const padding = 1;

  if (points.length < 2) return null;

  const max = Math.max(...points, 1);
  const min = Math.min(...points, 0);
  const range = max - min || 1;

  const xStep = (width - padding * 2) / (points.length - 1);

  const pathPoints = points.map((val, i) => {
    const x = padding + i * xStep;
    const y =
      height - padding - ((val - min) / range) * (height - padding * 2);
    return `${x},${y}`;
  });

  const pathD = `M ${pathPoints.join(' L ')}`;
  const lastPoint = points[points.length - 1];
  const trend = lastPoint >= points[0] ? '#22C55E' : '#EF4444';

  return (
    <svg
      width={width}
      height={height}
      className="flex-shrink-0"
      style={{ opacity: 0.7 }}
    >
      <path
        d={pathD}
        fill="none"
        stroke={trend}
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle
        cx={padding + (points.length - 1) * xStep}
        cy={
          height - padding - ((lastPoint - min) / range) * (height - padding * 2)
        }
        r="2"
        fill={trend}
      />
    </svg>
  );
}

function generateSparklinePoints(sunId: string): number[] {
  // Deterministic pseudo-random based on sunId for visual variety
  // In production, this would come from actual sun_runs history
  let hash = 0;
  for (let i = 0; i < sunId.length; i++) {
    hash = ((hash << 5) - hash + sunId.charCodeAt(i)) | 0;
  }
  const points: number[] = [];
  for (let i = 0; i < 7; i++) {
    hash = ((hash * 1103515245 + 12345) & 0x7fffffff);
    points.push(0.3 + (hash % 70) / 100);
  }
  return points;
}
