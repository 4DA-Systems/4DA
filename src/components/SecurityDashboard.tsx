import { useState, useMemo, useCallback, memo } from 'react';

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

// ============================================================================
// Mock Data
// ============================================================================

const INITIAL_ALERTS: SecurityAlert[] = [
  {
    id: 'sa-1',
    cveId: 'CVE-2026-10421',
    packageName: 'lodash',
    severity: 'critical',
    affectedProjects: 3,
    description: 'Prototype pollution via crafted object in lodash.merge',
  },
  {
    id: 'sa-2',
    cveId: 'CVE-2026-08855',
    packageName: 'xml2js',
    severity: 'high',
    affectedProjects: 1,
    description: 'XML external entity injection when parsing untrusted input',
  },
  {
    id: 'sa-3',
    cveId: 'CVE-2026-07233',
    packageName: 'semver',
    severity: 'medium',
    affectedProjects: 2,
    description: 'ReDoS in semver range parsing with crafted version strings',
  },
];

const INITIAL_RESOLVED: SecurityAlert[] = [
  {
    id: 'sa-r1',
    cveId: 'CVE-2025-44102',
    packageName: 'tar',
    severity: 'high',
    affectedProjects: 1,
    description: 'Arbitrary file overwrite via symlink following',
    resolvedAt: '2026-03-12',
  },
  {
    id: 'sa-r2',
    cveId: 'CVE-2025-38901',
    packageName: 'node-fetch',
    severity: 'medium',
    affectedProjects: 2,
    description: 'Header injection via carriage return in URL',
    resolvedAt: '2026-03-05',
  },
];

// ============================================================================
// Color mapping
// ============================================================================

const SEVERITY_BADGE: Record<Severity, string> = {
  critical: 'bg-[#EF4444]/15 text-[#EF4444] border-[#EF4444]/25',
  high: 'bg-[#F97316]/15 text-[#F97316] border-[#F97316]/25',
  medium: 'bg-[#D4AF37]/15 text-[#D4AF37] border-[#D4AF37]/25',
  low: 'bg-white/5 text-text-muted border-border',
};

const SEVERITY_DOT: Record<Severity, string> = {
  critical: 'bg-[#EF4444]',
  high: 'bg-[#F97316]',
  medium: 'bg-[#D4AF37]',
  low: 'bg-text-muted',
};

// ============================================================================
// Sub-components
// ============================================================================

function SeverityCount({ severity, count }: { severity: Severity; count: number }) {
  const labels: Record<Severity, string> = {
    critical: 'Critical',
    high: 'High',
    medium: 'Medium',
    low: 'Low',
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

// ============================================================================
// SecurityDashboard
// ============================================================================

const SecurityDashboard = memo(function SecurityDashboard() {
  const [activeAlerts, setActiveAlerts] = useState<SecurityAlert[]>(INITIAL_ALERTS);
  const [resolvedAlerts, setResolvedAlerts] = useState<SecurityAlert[]>(INITIAL_RESOLVED);

  const severityCounts = useMemo(() => {
    const counts: Record<Severity, number> = { critical: 0, high: 0, medium: 0, low: 0 };
    for (const a of activeAlerts) {
      counts[a.severity]++;
    }
    return counts;
  }, [activeAlerts]);

  const handleResolve = useCallback((alertId: string) => {
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

  const isEmpty = activeAlerts.length === 0;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">Security Alerts</h3>
        <p className="text-xs text-text-muted mt-1">
          Real-time vulnerability monitoring from the Developer Immune System.
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

        {/* Active Alerts */}
        {isEmpty ? (
          <div className="bg-[#22C55E]/5 rounded-lg border border-[#22C55E]/20 p-6 text-center">
            <div className="w-10 h-10 mx-auto mb-3 bg-[#22C55E]/10 rounded-full flex items-center justify-center">
              <span className="text-[#22C55E] text-lg">&#x2713;</span>
            </div>
            <p className="text-sm text-[#22C55E]">
              No vulnerabilities detected. Your dependencies are secure.
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
                  <div className="flex items-center gap-3 shrink-0 ml-3">
                    <span className="text-xs text-text-muted">
                      {alert.affectedProjects} project{alert.affectedProjects !== 1 ? 's' : ''}
                    </span>
                    <button
                      onClick={() => handleResolve(alert.id)}
                      className="px-3 py-1 text-xs bg-bg-tertiary border border-border rounded-lg text-text-secondary hover:text-white hover:border-text-muted/50 transition-colors"
                    >
                      Resolve
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
              Resolved
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
