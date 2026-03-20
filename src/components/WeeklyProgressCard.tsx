import { memo, useState, useEffect } from 'react';
import { cmd } from '../lib/commands';

function isWithinMondayWindow(): boolean {
  const now = new Date();
  const day = now.getDay();
  if (day === 1) return true; // Monday
  if (day === 2 && now.getHours() < 12) return true; // Tuesday morning grace
  return false;
}

interface Metric {
  label: string;
  value: string;
  delta?: number;
}

export const WeeklyProgressCard = memo(function WeeklyProgressCard() {
  const [metrics, setMetrics] = useState<Metric[] | null>(null);

  useEffect(() => {
    if (!isWithinMondayWindow()) return;

    cmd('get_intelligence_report', { period: 'week' })
      .then((data) => {
        const topTopic = data.topics_added > 0 ? `${data.topics_tracked} topics` : 'Exploring';
        setMetrics([
          { label: 'Items Scored', value: String(data.noise_rejected + data.feedback_signals) },
          { label: 'Relevant', value: String(data.feedback_signals), delta: data.accuracy_delta },
          { label: 'Accuracy', value: `${Math.round(data.accuracy_current)}%`, delta: data.accuracy_delta },
          { label: 'Focus', value: topTopic },
        ]);
      })
      .catch(() => { /* silently skip if unavailable */ });
  }, []);

  if (!metrics) return null;

  return (
    <div className="p-4 bg-[#141414] border border-[#2A2A2A] rounded-xl">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-semibold text-[#A0A0A0] uppercase tracking-wider">Weekly Progress</h3>
        <span className="text-[10px] text-[#8A8A8A]">This week</span>
      </div>
      <div className="grid grid-cols-4 gap-3">
        {metrics.map((m) => (
          <div key={m.label} className="p-2.5 bg-[#0A0A0A] rounded-lg border border-[#2A2A2A]">
            <p className="text-[10px] text-[#8A8A8A] mb-1">{m.label}</p>
            <div className="flex items-baseline gap-1.5">
              <span className="text-base font-semibold text-white tabular-nums">{m.value}</span>
              {m.delta != null && m.delta !== 0 && (
                <span className={`text-[10px] font-medium ${m.delta > 0 ? 'text-[#22C55E]' : 'text-[#EF4444]'}`}>
                  {m.delta > 0 ? '+' : ''}{m.delta.toFixed(1)}%
                </span>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
});
