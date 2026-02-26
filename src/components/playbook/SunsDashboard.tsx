import { useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import { SunModuleGroup } from './SunModuleGroup';
import type { SunAlert } from '../../store/suns-slice';

// ============================================================================
// Constants
// ============================================================================

const MODULE_NAMES: Record<string, string> = {
  S: 'Sovereignty',
  T: 'Technology',
  R: 'Revenue',
  E1: 'Execution',
  E2: 'Ecosystem',
  T2: 'Traction',
  S2: 'Scale',
};

const MODULE_ORDER = ['S', 'T', 'R', 'E1', 'E2', 'T2', 'S2'];

// ============================================================================
// Helpers
// ============================================================================

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
  const { t } = useTranslation();
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
        {t('action.dismiss')}
      </button>
    </div>
  );
}

// ============================================================================
// SunsDashboard
// ============================================================================

export function SunsDashboard() {
  const { t } = useTranslation();
  const {
    sunStatuses,
    sunAlerts,
    sunsLoading,
    streetHealth,
  } = useAppStore(
    useShallow((s) => ({
      sunStatuses: s.sunStatuses,
      sunAlerts: s.sunAlerts,
      sunsLoading: s.sunsLoading,
      streetHealth: s.streetHealth,
    })),
  );

  const loadStatuses = useAppStore((s) => s.loadSunStatuses);
  const loadAlerts = useAppStore((s) => s.loadSunAlerts);
  const loadStreetHealth = useAppStore((s) => s.loadStreetHealth);
  const toggleSun = useAppStore((s) => s.toggleSun);
  const acknowledgeSunAlert = useAppStore((s) => s.acknowledgeSunAlert);
  const triggerSun = useAppStore((s) => s.triggerSun);

  useEffect(() => {
    loadStatuses();
    loadAlerts();
    loadStreetHealth();

    const timer = setInterval(() => {
      loadStatuses();
      loadAlerts();
    }, 30000);

    return () => clearInterval(timer);
  }, [loadStatuses, loadAlerts, loadStreetHealth]);

  const handleTrigger = useCallback(async (sunId: string) => {
    await triggerSun(sunId);
  }, [triggerSun]);

  // Group suns by module_id, ordered by MODULE_ORDER
  const moduleGroups = useMemo(() => {
    const grouped = new Map<string, typeof sunStatuses>();
    for (const sun of sunStatuses) {
      const existing = grouped.get(sun.module_id) || [];
      existing.push(sun);
      grouped.set(sun.module_id, existing);
    }
    return MODULE_ORDER
      .filter((id) => grouped.has(id))
      .map((id) => ({
        moduleId: id,
        moduleName: MODULE_NAMES[id] || id,
        suns: grouped.get(id)!,
        health: streetHealth?.module_scores.find((m) => m.module_id === id),
      }));
  }, [sunStatuses, streetHealth]);

  const activeCount = sunStatuses.filter(s => s.enabled).length;
  const alertCount = sunAlerts.length;

  return (
    <div className="rounded-xl p-4 space-y-4"
      style={{ background: '#0A0A0A', border: '1px solid #2A2A2A' }}
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h3 className="text-base font-semibold" style={{ color: '#FFFFFF' }}>
            {t('suns.title')}
          </h3>
          <span className="text-xs px-1.5 py-0.5 rounded"
            style={{ background: '#D4AF3720', color: '#D4AF37' }}
          >
            {t('suns.active', { active: activeCount, total: sunStatuses.length })}
          </span>
          {sunsLoading && (
            <span className="text-xs" style={{ color: '#666666' }}>{t('action.loading')}</span>
          )}
        </div>
        {alertCount > 0 && (
          <span className="text-xs px-2 py-0.5 rounded-full"
            style={{ background: 'rgba(239,68,68,0.15)', color: '#EF4444' }}
          >
            {alertCount === 1 ? t('suns.alert', { count: alertCount }) : t('suns.alerts', { count: alertCount })}
          </span>
        )}
      </div>

      {/* Alerts */}
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

      {/* Module groups */}
      <div className="space-y-2">
        {moduleGroups.map(({ moduleId, moduleName, suns, health }) => (
          <SunModuleGroup
            key={moduleId}
            moduleId={moduleId}
            moduleName={moduleName}
            moduleHealth={health}
            suns={suns}
            onToggle={toggleSun}
            onTrigger={handleTrigger}
          />
        ))}
      </div>
    </div>
  );
}
