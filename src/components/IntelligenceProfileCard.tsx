import { useAppStore } from '../store';

export function IntelligenceProfileCard() {
  const learnedAffinities = useAppStore(s => s.learnedAffinities);
  const pulse = useAppStore(s => s.intelligencePulse);

  if (learnedAffinities.length === 0 && (!pulse || pulse.total_cycles === 0)) {
    return null;
  }

  const positiveAffinities = learnedAffinities.filter(a => a.affinity_score > 0);

  return (
    <div className="bg-[#1F1F1F] rounded-lg border border-border p-5">
      <h3 className="text-sm font-medium text-white mb-3">Your Intelligence Profile</h3>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Top Affinities */}
        <div>
          <span className="text-[10px] text-text-muted uppercase tracking-wider">Top Affinities</span>
          {positiveAffinities.length > 0 ? (
            <div className="mt-1.5 space-y-1">
              {positiveAffinities.slice(0, 3).map(a => (
                <div key={a.topic} className="flex items-center gap-2">
                  <span className="text-xs text-white truncate flex-1">{a.topic}</span>
                  <div className="w-12 h-1 bg-bg-tertiary rounded-full overflow-hidden flex-shrink-0">
                    <div
                      className="h-full bg-[#D4AF37] rounded-full"
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
            {positiveAffinities.length}
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
              <p className="text-xs text-text-secondary">{pulse.items_surfaced_7d} surfaced for you</p>
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
