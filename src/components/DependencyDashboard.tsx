import { useState, useMemo, memo } from 'react';

// ============================================================================
// Types
// ============================================================================

type Severity = 'critical' | 'high' | 'medium' | 'low';
type Freshness = 'fresh' | 'aging' | 'stale' | 'outdated';

interface Dependency {
  name: string;
  version: string;
  ecosystem: string;
  freshness: Freshness;
  alerts: DependencyAlert[];
}

interface DependencyAlert {
  id: string;
  severity: Severity;
  message: string;
  dependency: string;
}

interface Project {
  name: string;
  path: string;
  dependencies: Dependency[];
}

// ============================================================================
// Mock Data (backend commands not wired yet)
// ============================================================================

const MOCK_PROJECTS: Project[] = [
  {
    name: '4DA',
    path: '/projects/4da',
    dependencies: [
      { name: 'react', version: '19.1.0', ecosystem: 'npm', freshness: 'fresh', alerts: [] },
      { name: 'tauri', version: '2.3.1', ecosystem: 'cargo', freshness: 'fresh', alerts: [] },
      { name: 'serde', version: '1.0.219', ecosystem: 'cargo', freshness: 'fresh', alerts: [] },
      { name: 'vite', version: '7.3.1', ecosystem: 'npm', freshness: 'fresh', alerts: [] },
      {
        name: 'lodash', version: '4.17.20', ecosystem: 'npm', freshness: 'stale',
        alerts: [{ id: 'a1', severity: 'critical', message: 'Prototype pollution in lodash < 4.17.21', dependency: 'lodash' }],
      },
      { name: 'tokio', version: '1.43.0', ecosystem: 'cargo', freshness: 'aging', alerts: [] },
      {
        name: 'anyhow', version: '1.0.75', ecosystem: 'cargo', freshness: 'aging',
        alerts: [{ id: 'a2', severity: 'medium', message: 'Minor performance regression fixed in 1.0.80+', dependency: 'anyhow' }],
      },
      { name: 'zustand', version: '5.0.3', ecosystem: 'npm', freshness: 'fresh', alerts: [] },
    ],
  },
  {
    name: 'game-compiler',
    path: '/projects/game-compiler',
    dependencies: [
      { name: 'logos', version: '0.14.0', ecosystem: 'cargo', freshness: 'fresh', alerts: [] },
      { name: 'wgpu', version: '24.0.1', ecosystem: 'cargo', freshness: 'fresh', alerts: [] },
      {
        name: 'lsp-server', version: '0.7.0', ecosystem: 'cargo', freshness: 'stale',
        alerts: [{ id: 'a3', severity: 'high', message: 'Outdated lsp-server may miss protocol features', dependency: 'lsp-server' }],
      },
    ],
  },
];

// ============================================================================
// Freshness color mapping
// ============================================================================

const FRESHNESS_COLORS: Record<Freshness, string> = {
  fresh: 'bg-[#22C55E]',
  aging: 'bg-[#D4AF37]',
  stale: 'bg-[#F97316]',
  outdated: 'bg-[#EF4444]',
};

const SEVERITY_COLORS: Record<Severity, string> = {
  critical: 'bg-[#EF4444]/15 text-[#EF4444] border-[#EF4444]/25',
  high: 'bg-[#F97316]/15 text-[#F97316] border-[#F97316]/25',
  medium: 'bg-[#D4AF37]/15 text-[#D4AF37] border-[#D4AF37]/25',
  low: 'bg-white/5 text-text-muted border-border',
};

// ============================================================================
// Sub-components
// ============================================================================

function StatCard({ label, value, color }: { label: string; value: number; color?: string }) {
  return (
    <div className="bg-bg-tertiary rounded-lg border border-border px-4 py-3">
      <div className={`text-xl font-semibold ${color ?? 'text-white'}`}>
        {value.toLocaleString()}
      </div>
      <div className="text-xs text-text-muted mt-0.5">{label}</div>
    </div>
  );
}

function SeverityBadge({ severity }: { severity: Severity }) {
  return (
    <span className={`text-xs px-2 py-0.5 rounded border font-medium ${SEVERITY_COLORS[severity]}`}>
      {severity}
    </span>
  );
}

// ============================================================================
// DependencyDashboard
// ============================================================================

const DependencyDashboard = memo(function DependencyDashboard() {
  const [selectedProject, setSelectedProject] = useState(0);

  const project = MOCK_PROJECTS[selectedProject];
  const deps = project.dependencies;

  const stats = useMemo(() => {
    const fresh = deps.filter(d => d.freshness === 'fresh').length;
    const stale = deps.filter(d => d.freshness === 'stale' || d.freshness === 'outdated').length;
    const vulnerable = deps.filter(d => d.alerts.length > 0).length;
    return { total: deps.length, fresh, stale, vulnerable };
  }, [deps]);

  const allAlerts = useMemo(
    () => deps.flatMap(d => d.alerts),
    [deps],
  );

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-white">Dependency Health</h3>
          <p className="text-xs text-text-muted mt-1">
            Monitor freshness and vulnerabilities across your projects.
          </p>
        </div>
        <select
          value={selectedProject}
          onChange={e => setSelectedProject(Number(e.target.value))}
          className="bg-bg-tertiary border border-border rounded-lg px-3 py-1.5 text-sm text-text-primary focus:outline-none focus:border-text-muted/50"
        >
          {MOCK_PROJECTS.map((p, i) => (
            <option key={p.path} value={i}>{p.name}</option>
          ))}
        </select>
      </div>

      <div className="p-5 space-y-5">
        {/* Summary Stats */}
        <div className="grid grid-cols-4 gap-3">
          <StatCard label="Total" value={stats.total} />
          <StatCard label="Fresh" value={stats.fresh} color="text-[#22C55E]" />
          <StatCard label="Stale" value={stats.stale} color="text-[#F97316]" />
          <StatCard label="Vulnerable" value={stats.vulnerable} color="text-[#EF4444]" />
        </div>

        {/* Dependency Table */}
        <div className="overflow-hidden rounded-lg border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                <th className="text-left px-4 py-2.5 font-medium">Name</th>
                <th className="text-left px-4 py-2.5 font-medium">Version</th>
                <th className="text-left px-4 py-2.5 font-medium">Ecosystem</th>
                <th className="text-left px-4 py-2.5 font-medium">Freshness</th>
                <th className="text-left px-4 py-2.5 font-medium">Alerts</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {deps.map(dep => (
                <tr key={dep.name} className="hover:bg-[#1A1A1A] transition-colors">
                  <td className="px-4 py-2.5 font-mono text-text-primary">{dep.name}</td>
                  <td className="px-4 py-2.5 text-text-secondary font-mono">{dep.version}</td>
                  <td className="px-4 py-2.5 text-text-muted">{dep.ecosystem}</td>
                  <td className="px-4 py-2.5">
                    <div className="flex items-center gap-2">
                      <div className={`w-2 h-2 rounded-full ${FRESHNESS_COLORS[dep.freshness]}`} />
                      <span className="text-text-secondary capitalize">{dep.freshness}</span>
                    </div>
                  </td>
                  <td className="px-4 py-2.5">
                    {dep.alerts.length > 0 ? (
                      <span className="text-xs text-[#EF4444]">{dep.alerts.length}</span>
                    ) : (
                      <span className="text-xs text-text-muted">--</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Active Alerts */}
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            Active Alerts
          </h4>
          {allAlerts.length === 0 ? (
            <div className="bg-bg-primary rounded-lg border border-border/50 p-4">
              <p className="text-xs text-text-muted">
                No alerts. All dependencies are healthy.
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {allAlerts.map(alert => (
                <div
                  key={alert.id}
                  className="flex items-center justify-between bg-bg-primary rounded-lg border border-border/50 px-4 py-3"
                >
                  <div className="flex items-center gap-3">
                    <SeverityBadge severity={alert.severity} />
                    <span className="text-sm text-text-primary">{alert.message}</span>
                  </div>
                  <span className="text-xs text-text-muted font-mono">{alert.dependency}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
});

export default DependencyDashboard;
