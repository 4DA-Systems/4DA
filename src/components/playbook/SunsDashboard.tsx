import { useState, useEffect, useCallback } from 'react';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import type { SunStatus, SunAlert } from '../../store/suns-slice';

// ============================================================================
// Helpers
// ============================================================================

const MODULE_COLORS: Record<string, string> = {
  S: '#D4AF37', // Sovereignty - gold
  R: '#22C55E', // Revenue - green
};

function formatInterval(secs: number): string {
  if (secs >= 604800) return `${Math.round(secs / 604800)}d`;
  if (secs >= 86400) return `${Math.round(secs / 86400)}d`;
  if (secs >= 3600) return `${Math.round(secs / 3600)}h`;
  if (secs >= 60) return `${Math.round(secs / 60)}m`;
  return `${secs}s`;
}

function formatCountdown(secs: number | null): string {
  if (secs === null) return 'Disabled';
  if (secs <= 0) return 'Due now';
  if (secs >= 86400) {
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    return `${d}d ${h}h`;
  }
  if (secs >= 3600) {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return `${h}h ${m}m`;
  }
  if (secs >= 60) return `${Math.floor(secs / 60)}m`;
  return `${secs}s`;
}

function timeAgo(isoStr: string | null): string {
  if (!isoStr) return 'Never';
  const diff = (Date.now() - new Date(isoStr).getTime()) / 1000;
  if (diff < 60) return 'Just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

// ============================================================================
// Alert Row
// ============================================================================

function AlertRow({ alert, onAcknowledge }: { alert: SunAlert; onAcknowledge: (id: number) => void }) {
  const isFailure = alert.alert_type === 'failure';

  return (
    <div className="flex items-center gap-3 px-3 py-2 rounded-lg"
      style={{ background: isFailure ? 'rgba(239,68,68,0.08)' : 'rgba(212,175,55,0.08)' }}
    >
      <span className="text-xs font-medium px-1.5 py-0.5 rounded"
        style={{
          color: isFailure ? '#EF4444' : '#D4AF37',
          background: isFailure ? 'rgba(239,68,68,0.15)' : 'rgba(212,175,55,0.15)',
        }}
      >
        {alert.alert_type}
      </span>
      <span className="text-sm flex-1" style={{ color: '#A0A0A0' }}>
        <span style={{ color: '#FFFFFF' }}>{alert.sun_id}</span>: {alert.message}
      </span>
      <span className="text-xs" style={{ color: '#666666' }}>{timeAgo(alert.created_at)}</span>
      <button
        onClick={() => onAcknowledge(alert.id)}
        className="text-xs px-2 py-0.5 rounded transition-colors"
        style={{ border: '1px solid #2A2A2A', color: '#A0A0A0' }}
        onMouseEnter={e => { e.currentTarget.style.borderColor = '#D4AF37'; e.currentTarget.style.color = '#FFFFFF'; }}
        onMouseLeave={e => { e.currentTarget.style.borderColor = '#2A2A2A'; e.currentTarget.style.color = '#A0A0A0'; }}
      >
        Dismiss
      </button>
    </div>
  );
}

// ============================================================================
// Sun Row
// ============================================================================

function SunRow({ sun, onToggle, onTrigger }: {
  sun: SunStatus;
  onToggle: (id: string, enabled: boolean) => void;
  onTrigger: (id: string) => void;
}) {
  const [triggering, setTriggering] = useState(false);

  const handleTrigger = useCallback(async () => {
    setTriggering(true);
    onTrigger(sun.id);
    // Brief delay for visual feedback
    setTimeout(() => setTriggering(false), 1500);
  }, [sun.id, onTrigger]);

  const moduleColor = MODULE_COLORS[sun.module_id] || '#A0A0A0';

  return (
    <div className="flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors"
      style={{ background: '#141414', border: '1px solid #2A2A2A' }}
    >
      {/* Module badge */}
      <span className="w-6 h-6 flex items-center justify-center rounded text-xs font-semibold flex-shrink-0"
        style={{ background: `${moduleColor}20`, color: moduleColor }}
      >
        {sun.module_id}
      </span>

      {/* Name and info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium" style={{ color: sun.enabled ? '#FFFFFF' : '#666666' }}>
            {sun.name}
          </span>
          <span className="text-xs" style={{ color: '#666666' }}>
            every {formatInterval(sun.interval_secs)}
          </span>
        </div>
        <div className="flex items-center gap-3 mt-0.5">
          <span className="text-xs" style={{ color: '#666666' }}>
            Last: {timeAgo(sun.last_run)}
          </span>
          {sun.enabled && (
            <span className="text-xs" style={{ color: '#A0A0A0' }}>
              Next: {formatCountdown(sun.next_run_in_secs)}
            </span>
          )}
          {sun.run_count > 0 && (
            <span className="text-xs" style={{ color: '#666666' }}>
              Runs: {sun.run_count}
            </span>
          )}
        </div>
        {sun.last_result && (
          <div className="text-xs mt-0.5 truncate" style={{ color: '#666666', maxWidth: '300px' }}>
            {sun.last_result}
          </div>
        )}
      </div>

      {/* Manual trigger */}
      <button
        onClick={handleTrigger}
        disabled={triggering || !sun.enabled}
        className="text-xs px-2 py-1 rounded transition-colors flex-shrink-0"
        style={{
          border: '1px solid #2A2A2A',
          color: triggering ? '#D4AF37' : sun.enabled ? '#A0A0A0' : '#666666',
          cursor: sun.enabled ? 'pointer' : 'not-allowed',
          opacity: sun.enabled ? 1 : 0.5,
        }}
        onMouseEnter={e => { if (sun.enabled) { e.currentTarget.style.borderColor = '#D4AF37'; e.currentTarget.style.color = '#FFFFFF'; }}}
        onMouseLeave={e => { e.currentTarget.style.borderColor = '#2A2A2A'; e.currentTarget.style.color = '#A0A0A0'; }}
        title="Run this sun now"
      >
        {triggering ? 'Running...' : 'Run'}
      </button>

      {/* Enable/disable toggle */}
      <button
        onClick={() => onToggle(sun.id, !sun.enabled)}
        className="w-9 h-5 rounded-full relative transition-colors flex-shrink-0"
        style={{
          background: sun.enabled ? '#22C55E' : '#2A2A2A',
        }}
        title={sun.enabled ? 'Disable' : 'Enable'}
      >
        <span
          className="absolute top-0.5 w-4 h-4 rounded-full transition-transform"
          style={{
            background: '#FFFFFF',
            left: sun.enabled ? '18px' : '2px',
          }}
        />
      </button>
    </div>
  );
}

// ============================================================================
// SunsDashboard
// ============================================================================

export function SunsDashboard() {
  const {
    sunStatuses,
    sunAlerts,
    sunsLoading,
  } = useAppStore(
    useShallow((s) => ({
      sunStatuses: s.sunStatuses,
      sunAlerts: s.sunAlerts,
      sunsLoading: s.sunsLoading,
    })),
  );

  const loadStatuses = useAppStore((s) => s.loadSunStatuses);
  const loadAlerts = useAppStore((s) => s.loadSunAlerts);
  const toggleSun = useAppStore((s) => s.toggleSun);
  const acknowledgeSunAlert = useAppStore((s) => s.acknowledgeSunAlert);
  const triggerSun = useAppStore((s) => s.triggerSun);

  useEffect(() => {
    loadStatuses();
    loadAlerts();

    // Refresh every 30s to update countdowns
    const timer = setInterval(() => {
      loadStatuses();
      loadAlerts();
    }, 30000);

    return () => clearInterval(timer);
  }, [loadStatuses, loadAlerts]);

  const handleTrigger = useCallback(async (sunId: string) => {
    await triggerSun(sunId);
  }, [triggerSun]);

  const activeCount = sunStatuses.filter(s => s.enabled).length;
  const alertCount = sunAlerts.length;

  return (
    <div className="rounded-xl p-4 space-y-4"
      style={{ background: '#141414', border: '1px solid #2A2A2A' }}
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h3 className="text-base font-semibold" style={{ color: '#FFFFFF' }}>
            Suns
          </h3>
          <span className="text-xs px-1.5 py-0.5 rounded"
            style={{ background: '#D4AF3720', color: '#D4AF37' }}
          >
            {activeCount}/{sunStatuses.length} active
          </span>
          {sunsLoading && (
            <span className="text-xs" style={{ color: '#666666' }}>Loading...</span>
          )}
        </div>
        {alertCount > 0 && (
          <span className="text-xs px-2 py-0.5 rounded-full"
            style={{ background: 'rgba(239,68,68,0.15)', color: '#EF4444' }}
          >
            {alertCount} alert{alertCount !== 1 ? 's' : ''}
          </span>
        )}
      </div>

      {/* Alerts section */}
      {sunAlerts.length > 0 && (
        <div className="space-y-1.5">
          {sunAlerts.map(alert => (
            <AlertRow
              key={alert.id}
              alert={alert}
              onAcknowledge={acknowledgeSunAlert}
            />
          ))}
        </div>
      )}

      {/* Sun list */}
      <div className="space-y-1.5">
        {sunStatuses.map(sun => (
          <SunRow
            key={sun.id}
            sun={sun}
            onToggle={toggleSun}
            onTrigger={handleTrigger}
          />
        ))}
      </div>

      {/* Legend */}
      <div className="flex items-center gap-4 pt-1">
        <div className="flex items-center gap-1.5">
          <span className="w-3 h-3 rounded text-[8px] font-bold flex items-center justify-center"
            style={{ background: '#D4AF3720', color: '#D4AF37' }}>S</span>
          <span className="text-xs" style={{ color: '#666666' }}>Sovereignty</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="w-3 h-3 rounded text-[8px] font-bold flex items-center justify-center"
            style={{ background: '#22C55E20', color: '#22C55E' }}>R</span>
          <span className="text-xs" style={{ color: '#666666' }}>Revenue</span>
        </div>
      </div>
    </div>
  );
}
