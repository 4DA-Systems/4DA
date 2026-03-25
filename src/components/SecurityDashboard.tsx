import { useState, useMemo, useCallback, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { FirstCveCard } from './FirstCveCard';
import { cmd } from '../lib/commands';

// ============================================================================
// Types
// ============================================================================

type Severity = 'critical' | 'high' | 'medium' | 'low';

interface SecurityAlert {
  id: string;
  cveId: string;
  packageName: string;
  severity: Severity;
  affectedProjects: number;
  description: string;
  resolvedAt?: string;
}

interface DepAlert {
  id: number;
  package_name: string;
  ecosystem: string;
  alert_type: string;
  severity: string;
  title: string;
  description: string | null;
  affected_versions: string | null;
  source_url: string | null;
  detected_at: string;
}

// ============================================================================
// Color mapping
// ============================================================================

const SEVERITY_BADGE: Record<Severity, string> = {
  critical: 'bg-error/15 text-error border-error/25',
  high: 'bg-[var(--color-accent-action)]/15 text-[var(--color-accent-action)] border-[var(--color-accent-action)]/25',
  medium: 'bg-accent-gold/15 text-accent-gold border-accent-gold/25',
  low: 'bg-white/5 text-text-muted border-border',
};

const SEVERITY_DOT: Record<Severity, string> = {
  critical: 'bg-error',
  high: 'bg-[var(--color-accent-action)]',
  medium: 'bg-accent-gold',
  low: 'bg-text-muted',
};

// ============================================================================
// Sub-components
// ============================================================================

function SeverityCount({ severity, count }: { severity: Severity; count: number }) {
  const { t } = useTranslation();
  const labels: Record<Severity, string> = {
    critical: t('security.severityCritical'),
    high: t('security.severityHigh'),
    medium: t('security.severityMedium'),
    low: t('security.severityLow'),
  };
  return (
    <div className="bg-bg-tertiary rounded-lg border border-border px-4 py-3 flex items-center gap-3">
      <div className={`w-2.5 h-2.5 rounded-full ${SEVERITY_DOT[severity]}`} />
      <div>
        <div className="text-lg font-semibold text-white">{count}</div>
        <div className="text-xs text-text-muted">{labels[severity]}</div>
      </div>
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <div className="h-4 bg-bg-tertiary rounded w-32 animate-pulse" />
        <div className="h-3 bg-bg-tertiary rounded w-64 mt-2 animate-pulse" />
      </div>
      <div className="p-5 space-y-4">
        <div className="grid grid-cols-4 gap-3">
          {[1, 2, 3, 4].map(i => (
            <div key={i} className="h-16 bg-bg-tertiary rounded-lg animate-pulse" />
          ))}
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Data fetching — loads from get_dependency_alerts (real dependency scanner)
// Falls back to empty state when no dependency scanning is configured
// ============================================================================

function mapDepAlertsToSecurityAlerts(depAlerts: DepAlert[]): SecurityAlert[] {
  return depAlerts.map(a => {
    const severity: Severity =
      (a.severity as Severity) in SEVERITY_BADGE ? a.severity as Severity : 'low';

    return {
      id: `da-${a.id}`,
      cveId: a.title.match(/CVE-\d{4}-\d+/)?.[0] ?? `DA-${a.id}`,
      packageName: a.package_name,
      severity,
      affectedProjects: 1,
      description: a.description ?? a.title,
    };
  });
}

// ============================================================================
// SecurityDashboard
// ============================================================================

const SecurityDashboard = memo(function SecurityDashboard() {
  const { t } = useTranslation();
  const [activeAlerts, setActiveAlerts] = useState<SecurityAlert[]>([]);
  const [resolvedAlerts, setResolvedAlerts] = useState<SecurityAlert[]>([]);
  const [loading, setLoading] = useState(true);
  const [configured, setConfigured] = useState(true);

  useEffect(() => {
    setLoading(true);
    cmd('get_dependency_alerts')
      .then(result => {
        const alerts = mapDepAlertsToSecurityAlerts(
          (result as { alerts: DepAlert[] }).alerts,
        );
        setActiveAlerts(alerts);
        setResolvedAlerts([]);
        setConfigured(true);
      })
      .catch(() => {
        setActiveAlerts([]);
        setResolvedAlerts([]);
        setConfigured(false);
      })
      .finally(() => setLoading(false));
  }, []);

  const severityCounts = useMemo(() => {
    const counts: Record<Severity, number> = { critical: 0, high: 0, medium: 0, low: 0 };
    for (const a of activeAlerts) {
      counts[a.severity]++;
    }
    return counts;
  }, [activeAlerts]);

  const handleResolve = useCallback((alertId: string) => {
    // Extract the dependency alert id and resolve it on the backend
    const numericId = parseInt(alertId.replace('da-', ''), 10);
    if (!isNaN(numericId)) {
      cmd('resolve_dependency_alert', { alertId: numericId }).catch(() => {
        // Best-effort — still remove from UI
      });
    }

    setActiveAlerts(prev => {
      const alert = prev.find(a => a.id === alertId);
      if (alert) {
        setResolvedAlerts(r => [
          { ...alert, resolvedAt: new Date().toISOString().slice(0, 10) },
          ...r,
        ]);
      }
      return prev.filter(a => a.id !== alertId);
    });
  }, []);

  if (loading) return <LoadingSkeleton />;

  const isEmpty = activeAlerts.length === 0;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">{t('security.title')}</h3>
        <p className="text-xs text-text-muted mt-1">
          {configured
            ? t('security.subtitle')
            : t('security.notConfigured')}
        </p>
      </div>

      <div className="p-5 space-y-5">
        {/* Severity Counts */}
        <div className="grid grid-cols-4 gap-3">
          <SeverityCount severity="critical" count={severityCounts.critical} />
          <SeverityCount severity="high" count={severityCounts.high} />
          <SeverityCount severity="medium" count={severityCounts.medium} />
          <SeverityCount severity="low" count={severityCounts.low} />
        </div>

        {/* First CVE highlight — self-manages visibility via localStorage */}
        {activeAlerts.length > 0 && (
          <FirstCveCard
            cveId={activeAlerts[0]!.cveId}
            packageName={activeAlerts[0]!.packageName}
            severity={activeAlerts[0]!.severity}
            projectCount={activeAlerts[0]!.affectedProjects}
            minutesSincePublication={0}
          />
        )}

        {/* Active Alerts */}
        {isEmpty ? (
          <div className="bg-success/5 rounded-lg border border-success/20 p-6 text-center">
            <div className="w-10 h-10 mx-auto mb-3 bg-success/10 rounded-full flex items-center justify-center">
              <span className="text-success text-lg">&#x2713;</span>
            </div>
            <p className="text-sm text-success">
              {configured
                ? t('security.noVulnerabilities')
                : t('security.monitoringInactive')}
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {activeAlerts.map(alert => (
              <div
                key={alert.id}
                className="bg-bg-primary rounded-lg border border-border/50 px-4 py-3"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3 min-w-0">
                    <span className={`text-xs px-2 py-0.5 rounded border font-medium shrink-0 ${SEVERITY_BADGE[alert.severity]}`}>
                      {alert.severity}
                    </span>
                    <span className="text-xs text-text-muted font-mono shrink-0">
                      {alert.cveId}
                    </span>
                    <span className="text-xs text-text-muted shrink-0">
                      {alert.packageName}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 shrink-0 ms-3">
                    <span className="text-xs text-text-muted">
                      {t('security.projectCount', { count: alert.affectedProjects })}
                    </span>
                    <button
                      onClick={() => handleResolve(alert.id)}
                      className="px-3 py-1 text-xs bg-bg-tertiary border border-border rounded-lg text-text-secondary hover:text-white hover:border-text-muted/50 transition-colors"
                    >
                      {t('security.resolve')}
                    </button>
                  </div>
                </div>
                <p className="text-xs text-text-secondary mt-2">{alert.description}</p>
              </div>
            ))}
          </div>
        )}

        {/* Resolved Timeline */}
        {resolvedAlerts.length > 0 && (
          <div>
            <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
              {t('security.resolved')}
            </h4>
            <div className="space-y-1.5">
              {resolvedAlerts.map(alert => (
                <div
                  key={alert.id}
                  className="flex items-center justify-between px-4 py-2.5 bg-bg-primary/50 rounded-lg border border-border/30"
                >
                  <div className="flex items-center gap-3">
                    <span className="text-xs text-text-muted font-mono">{alert.cveId}</span>
                    <span className="text-xs text-text-muted">{alert.packageName}</span>
                    <span className="text-xs text-text-muted/50 line-through">
                      {alert.severity}
                    </span>
                  </div>
                  <span className="text-xs text-text-muted">
                    {alert.resolvedAt}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

export default SecurityDashboard;
