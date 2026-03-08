import { useAppStore } from '../store';

export function IntelligenceProfileCard() {
  const learnedAffinities = useAppStore(s => s.learnedAffinities) ?? [];
  const pulse = useAppStore(s => s.intelligencePulse);

  if (learnedAffinities.length === 0 && (!pulse || pulse.total_cycles === 0)) {
    return null;
  }

  const positiveAffinities = learnedAffinities.filter(a => a.affinity_score > 0);
  const topByStrength = [...learnedAffinities]
    .sort((a, b) => Math.abs(b.affinity_score) - Math.abs(a.affinity_score))
    .slice(0, 3);
  const displayAffinities = positiveAffinities.length > 0 ? positiveAffinities.slice(0, 3) : topByStrength;

  return (
    <div className="bg-[#1F1F1F] rounded-lg border border-border p-5">
      <h3 className="text-sm font-medium text-white mb-3">Your Intelligence Profile</h3>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Top Affinities */}
        <div>
          <span className="text-[10px] text-text-muted uppercase tracking-wider">
            {positiveAffinities.length > 0 ? 'Top Affinities' : 'Strongest Signals'}
          </span>
          {displayAffinities.length > 0 ? (
            <div className="mt-1.5 space-y-1">
              {displayAffinities.map(a => (
                <div key={a.topic} className="flex items-center gap-2">
                  <span className="text-xs text-white truncate flex-1">{a.topic}</span>
                  <div className="w-12 h-1 bg-bg-tertiary rounded-full overflow-hidden flex-shrink-0">
                    <div
                      className={`h-full rounded-full ${a.affinity_score > 0 ? 'bg-[#D4AF37]' : 'bg-[#666666]'}`}
                      style={{ width: `${Math.min(Math.abs(a.affinity_score) * 100, 100)}%` }}
                    />
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-[10px] text-text-muted mt-1.5">Interact with results to build affinities</p>
          )}
        </div>
        {/* Learning Velocity */}
        <div>
          <span className="text-[10px] text-text-muted uppercase tracking-wider">Learning Velocity</span>
          <p className="text-lg font-semibold text-white mt-1">
            {learnedAffinities.length}
            <span className="text-xs font-normal text-text-muted ml-1">topics learned</span>
          </p>
          {pulse && pulse.calibration_accuracy > 0 && (
            <p className="text-[10px] text-text-muted mt-0.5">
              {Math.round(pulse.calibration_accuracy * 100)}% calibration accuracy
            </p>
          )}
        </div>
        {/* System Activity */}
        <div>
          <span className="text-[10px] text-text-muted uppercase tracking-wider">System Activity</span>
          {pulse ? (
            <div className="mt-1.5 space-y-1">
              <p className="text-xs text-white">{pulse.items_analyzed_7d.toLocaleString()} items analyzed (7d)</p>
              <p className="text-xs text-text-secondary">
                {pulse.items_surfaced_7d > 0
                  ? `${pulse.items_surfaced_7d} marked relevant`
                  : pulse.items_analyzed_7d > 0
                    ? 'Analyzing your preferences'
                    : '0 marked relevant'}
              </p>
              <p className="text-xs text-text-muted">{pulse.total_cycles} learning cycles complete</p>
            </div>
          ) : (
            <p className="text-[10px] text-text-muted mt-1.5">Analysis data will appear after first cycle</p>
          )}
        </div>
      </div>
    </div>
  );
}
