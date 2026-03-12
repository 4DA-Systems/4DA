import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

export function TeamSignalFeed() {
  const { t } = useTranslation();
  const teamSignals = useAppStore(s => s.teamSignals);
  const teamSignalsLoading = useAppStore(s => s.teamSignalsLoading);
  const loadTeamSignals = useAppStore(s => s.loadTeamSignals);
  const resolveTeamSignal = useAppStore(s => s.resolveTeamSignal);

  const [showResolved, setShowResolved] = useState(false);
  const [resolvingId, setResolvingId] = useState<string | null>(null);
  const [resolveNotes, setResolveNotes] = useState('');

  useEffect(() => {
    loadTeamSignals(showResolved);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showResolved]);

  const activeSignals = teamSignals.filter(s => !s.resolved);
  const resolvedSignals = teamSignals.filter(s => s.resolved);
  const displaySignals = showResolved ? teamSignals : activeSignals;

  const handleResolve = async (signalId: string) => {
    await resolveTeamSignal(signalId, resolveNotes);
    setResolvingId(null);
    setResolveNotes('');
  };

  if (teamSignalsLoading && teamSignals.length === 0) {
    return (
      <div className="flex items-center justify-center py-6">
        <span className="text-xs text-text-muted">{t('action.loading', 'Loading...')}</span>
      </div>
    );
  }

  if (teamSignals.length === 0) {
    return (
      <div className="space-y-2">
        <p className="text-xs text-text-muted text-center py-6">
          {t('team.signals.empty', 'No shared signals yet. Team signals appear here when members share discoveries.')}
        </p>
        <p className="text-[10px] text-text-muted text-center">
          {t('team.signals.hint', 'Share signals from your Signal Chains view, or enable auto-sharing in Team Settings.')}
        </p>
      </div>
    );
  }

  const severityColor = (sev: string) => {
    switch (sev) {
      case 'critical': return 'bg-[#EF4444]/15 text-[#EF4444]';
      case 'high': return 'bg-[#F97316]/15 text-[#F97316]';
      case 'medium': return 'bg-[#D4AF37]/15 text-[#D4AF37]';
      default: return 'bg-border text-text-muted';
    }
  };

  return (
    <div className="space-y-2">
      {/* Toggle */}
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-text-muted">
          {activeSignals.length} {t('team.signals.active', 'active')}
          {resolvedSignals.length > 0 && `, ${resolvedSignals.length} ${t('team.signals.resolved', 'resolved')}`}
        </span>
        <button
          onClick={() => setShowResolved(!showResolved)}
          className="text-[10px] text-text-muted hover:text-text-secondary transition-colors"
        >
          {showResolved ? t('team.signals.hideResolved', 'Hide resolved') : t('team.signals.showResolved', 'Show resolved')}
        </button>
      </div>

      {/* Signal Cards */}
      {displaySignals.map(signal => (
        <div
          key={signal.id}
          className={`px-3 py-2.5 rounded-lg border ${
            signal.resolved
              ? 'bg-bg-primary/50 border-border/30 opacity-60'
              : 'bg-bg-primary border-border/50'
          }`}
        >
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs text-white font-medium truncate">{signal.title}</span>
            <span className={`text-[10px] px-1.5 py-0.5 rounded ${severityColor(signal.severity)}`}>
              {signal.severity}
            </span>
          </div>

          <div className="flex items-center gap-2 text-[10px] text-text-muted">
            <span>{signal.signal_type}</span>
            <span>&#183;</span>
            <span>{signal.detected_by_count} {t('team.signals.detectors', 'detectors')}</span>
            {signal.tech_topics.length > 0 && (
              <>
                <span>&#183;</span>
                <span className="truncate">{signal.tech_topics.slice(0, 2).join(', ')}</span>
              </>
            )}
          </div>

          {signal.resolved ? (
            <p className="text-[10px] text-[#22C55E] mt-1">
              {t('team.signals.resolvedBy', 'Resolved')}
            </p>
          ) : (
            <div className="mt-1.5">
              {resolvingId === signal.id ? (
                <div className="flex items-center gap-1.5">
                  <input
                    type="text"
                    value={resolveNotes}
                    onChange={e => setResolveNotes(e.target.value)}
                    placeholder={t('team.signals.resolveNotes', 'Resolution notes...')}
                    className="flex-1 px-2 py-1 text-[10px] bg-bg-tertiary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
                    onKeyDown={e => e.key === 'Enter' && handleResolve(signal.id)}
                    autoFocus
                  />
                  <button
                    onClick={() => handleResolve(signal.id)}
                    className="text-[10px] px-2 py-1 bg-[#22C55E]/15 text-[#22C55E] rounded hover:bg-[#22C55E]/25 transition-colors"
                  >
                    {t('action.resolve', 'Resolve')}
                  </button>
                  <button
                    onClick={() => { setResolvingId(null); setResolveNotes(''); }}
                    className="text-[10px] text-text-muted hover:text-white transition-colors"
                  >
                    {t('action.cancel', 'Cancel')}
                  </button>
                </div>
              ) : (
                <button
                  onClick={() => setResolvingId(signal.id)}
                  className="text-[10px] text-text-muted hover:text-[#22C55E] transition-colors"
                >
                  {t('action.resolve', 'Resolve')}
                </button>
              )}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
