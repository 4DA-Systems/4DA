import { useState, useEffect, useMemo, useCallback, memo } from 'react';
import { cmd } from '../lib/commands';

// ============================================================================
// Types
// ============================================================================

type Severity = 'critical' | 'high' | 'medium' | 'low';

interface DepEntry {
  name: string;
  version: string | null;
  ecosystem: string;
  is_dev: boolean;
  alerts: Array<{ id: number; severity: string; title: string }>;
}

interface ProjectEntry {
  name: string;
  path: string;
  dependency_count: number;
  alert_count: number;
}

interface OverviewData {
  total_dependencies: number;
  total_projects: number;
  direct_dependencies: number;
  dev_dependencies: number;
  ecosystems: Array<{ ecosystem: string; count: number }>;
  projects: ProjectEntry[];
  alerts: { total: number; critical: number; high: number; medium: number; low: number };
  cross_project_packages: number;
  cross_project_top: Array<{ package_name: string; ecosystem: string; project_count: number }>;
}

// ============================================================================
// Constants
// ============================================================================

const SEVERITY_COLORS: Record<Severity, string> = {
  critical: 'bg-[#EF4444]/15 text-[#EF4444] border-[#EF4444]/25',
  high: 'bg-[#F97316]/15 text-[#F97316] border-[#F97316]/25',
  medium: 'bg-[#D4AF37]/15 text-[#D4AF37] border-[#D4AF37]/25',
  low: 'bg-white/5 text-text-muted border-border',
};

const ECOSYSTEM_COLORS: Record<string, string> = {
  rust: 'text-[#DEA584]',
  javascript: 'text-[#F7DF1E]',
  typescript: 'text-[#3178C6]',
  python: 'text-[#3776AB]',
  go: 'text-[#00ADD8]',
  java: 'text-[#ED8B00]',
  ruby: 'text-[#CC342D]',
  php: 'text-[#777BB4]',
  dart: 'text-[#0175C2]',
  cpp: 'text-[#00599C]',
  csharp: 'text-[#239120]',
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

function SeverityBadge({ severity }: { severity: string }) {
  const sev = (severity as Severity) in SEVERITY_COLORS ? severity as Severity : 'low';
  return (
    <span className={`text-xs px-2 py-0.5 rounded border font-medium ${SEVERITY_COLORS[sev]}`}>
      {severity}
    </span>
  );
}

function EcosystemBadge({ ecosystem }: { ecosystem: string }) {
  const color = ECOSYSTEM_COLORS[ecosystem] ?? 'text-text-muted';
  return (
    <span className={`text-xs font-mono ${color}`}>{ecosystem}</span>
  );
}

function LoadingSkeleton() {
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <div className="h-4 bg-bg-tertiary rounded w-36 animate-pulse" />
        <div className="h-3 bg-bg-tertiary rounded w-64 mt-2 animate-pulse" />
      </div>
      <div className="p-5 space-y-4">
        <div className="grid grid-cols-4 gap-3">
          {[1, 2, 3, 4].map(i => (
            <div key={i} className="h-16 bg-bg-tertiary rounded-lg animate-pulse" />
          ))}
        </div>
        <div className="h-48 bg-bg-tertiary rounded-lg animate-pulse" />
      </div>
    </div>
  );
}

// ============================================================================
// DependencyDashboard
// ============================================================================

interface AceScanSummary {
  projects_scanned: number;
  total_dependencies: number;
  primary_stack: string;
  languages: string[];
  frameworks: string[];
  has_data: boolean;
}

const DependencyDashboard = memo(function DependencyDashboard() {
  const [overview, setOverview] = useState<OverviewData | null>(null);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [projectDeps, setProjectDeps] = useState<DepEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingProject, setLoadingProject] = useState(false);
  const [scanSummary, setScanSummary] = useState<AceScanSummary | null>(null);

  // Fetch overview on mount
  useEffect(() => {
    setLoading(true);
    cmd('get_dependency_overview')
      .then(data => {
        setOverview(data);
        // Auto-select first project if available
        if (data.projects.length > 0) {
          setSelectedProject(data.projects[0].path);
        }
      })
      .catch(() => setOverview(null))
      .finally(() => setLoading(false));
  }, []);

  // Fetch project dependencies when selection changes
  useEffect(() => {
    if (!selectedProject) {
      setProjectDeps([]);
      return;
    }
    setLoadingProject(true);
    cmd('get_project_deps', { projectPath: selectedProject })
      .then(result => {
        setProjectDeps(result.dependencies as DepEntry[]);
      })
      .catch(() => setProjectDeps([]))
      .finally(() => setLoadingProject(false));
  }, [selectedProject]);

  const handleResolveAlert = useCallback(async (alertId: number) => {
    try {
      await cmd('resolve_dependency_alert', { alertId });
      // Refresh overview
      const data = await cmd('get_dependency_overview');
      setOverview(data);
      // Refresh project deps if selected
      if (selectedProject) {
        const result = await cmd('get_project_deps', { projectPath: selectedProject });
        setProjectDeps(result.dependencies as DepEntry[]);
      }
    } catch {
      // Silently fail — alert may already be resolved
    }
  }, [selectedProject]);

  const activeProject = useMemo(() => {
    if (!overview || !selectedProject) return null;
    return overview.projects.find(p => p.path === selectedProject) ?? null;
  }, [overview, selectedProject]);

  // Fetch ACE scan summary when no dependency data is available
  useEffect(() => {
    if (!loading && (!overview || overview.total_dependencies === 0)) {
      cmd('ace_get_scan_summary')
        .then((data) => setScanSummary(data as unknown as AceScanSummary))
        .catch(() => {});
    }
  }, [loading, overview]);

  if (loading) return <LoadingSkeleton />;

  if (!overview || overview.total_dependencies === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border">
          <h3 className="text-sm font-medium text-white">Dependency Intelligence</h3>
          <p className="text-xs text-text-muted mt-1">
            Track dependencies across all your projects.
          </p>
        </div>
        <div className="p-5">
          {scanSummary && scanSummary.has_data ? (
            <div className="bg-bg-primary rounded-lg border border-border/50 p-6 text-center">
              <div className="flex items-center justify-center gap-2 mb-2">
                <div className="w-2 h-2 rounded-full bg-accent-gold/60 animate-pulse" />
                <p className="text-sm text-text-secondary">
                  4DA detected: {scanSummary.primary_stack || scanSummary.languages.join(', ')}
                </p>
              </div>
              <p className="text-xs text-text-muted mb-4">
                {scanSummary.total_dependencies} dependencies across {scanSummary.projects_scanned} project{scanSummary.projects_scanned !== 1 ? 's' : ''}
              </p>
              <p className="text-xs text-text-muted/60 leading-relaxed">
                Run a dependency scan to unlock vulnerability tracking and cross-project analysis.
              </p>
            </div>
          ) : (
            <div className="bg-bg-primary rounded-lg border border-border/50 p-6 text-center">
              <p className="text-sm text-text-muted mb-2">No dependency data available</p>
              <p className="text-xs text-text-muted/60 leading-relaxed">
                Run a context scan to detect your projects and dependencies.
                4DA will index them automatically from your manifests.
              </p>
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-white">Dependency Intelligence</h3>
          <p className="text-xs text-text-muted mt-1">
            {overview.total_projects} project{overview.total_projects !== 1 ? 's' : ''},{' '}
            {overview.total_dependencies} dependencies tracked
          </p>
        </div>
        {overview.projects.length > 1 && (
          <select
            value={selectedProject ?? ''}
            onChange={e => setSelectedProject(e.target.value)}
            className="bg-bg-tertiary border border-border rounded-lg px-3 py-1.5 text-sm text-text-primary focus:outline-none focus:border-text-muted/50"
          >
            {overview.projects.map(p => (
              <option key={p.path} value={p.path}>
                {p.name} ({p.dependency_count})
              </option>
            ))}
          </select>
        )}
      </div>

      <div className="p-5 space-y-5">
        {/* Summary Stats */}
        <div className="grid grid-cols-4 gap-3">
          <StatCard label="Total" value={overview.total_dependencies} />
          <StatCard label="Direct" value={overview.direct_dependencies} color="text-[#22C55E]" />
          <StatCard label="Dev" value={overview.dev_dependencies} color="text-[#D4AF37]" />
          <StatCard
            label="Alerts"
            value={overview.alerts.total}
            color={overview.alerts.total > 0 ? 'text-[#EF4444]' : 'text-[#22C55E]'}
          />
        </div>

        {/* Ecosystem Breakdown */}
        {overview.ecosystems.length > 0 && (
          <div className="flex gap-3 flex-wrap">
            {overview.ecosystems.map(eco => (
              <div
                key={eco.ecosystem}
                className="bg-bg-tertiary rounded-lg border border-border px-3 py-2 flex items-center gap-2"
              >
                <EcosystemBadge ecosystem={eco.ecosystem} />
                <span className="text-xs text-text-secondary">{eco.count}</span>
              </div>
            ))}
          </div>
        )}

        {/* Dependency Table for Selected Project */}
        {activeProject && (
          <div>
            <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
              {activeProject.name} Dependencies
            </h4>
            {loadingProject ? (
              <div className="h-32 bg-bg-tertiary rounded-lg animate-pulse" />
            ) : projectDeps.length === 0 ? (
              <div className="bg-bg-primary rounded-lg border border-border/50 p-4">
                <p className="text-xs text-text-muted">No dependencies found for this project.</p>
              </div>
            ) : (
              <div className="overflow-hidden rounded-lg border border-border">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                      <th className="text-left px-4 py-2.5 font-medium">Name</th>
                      <th className="text-left px-4 py-2.5 font-medium">Version</th>
                      <th className="text-left px-4 py-2.5 font-medium">Ecosystem</th>
                      <th className="text-left px-4 py-2.5 font-medium">Type</th>
                      <th className="text-left px-4 py-2.5 font-medium">Alerts</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {projectDeps.map(dep => (
                      <tr key={`${dep.name}-${dep.ecosystem}`} className="hover:bg-[#1A1A1A] transition-colors">
                        <td className="px-4 py-2.5 font-mono text-text-primary">{dep.name}</td>
                        <td className="px-4 py-2.5 text-text-secondary font-mono">
                          {dep.version || '--'}
                        </td>
                        <td className="px-4 py-2.5">
                          <EcosystemBadge ecosystem={dep.ecosystem} />
                        </td>
                        <td className="px-4 py-2.5">
                          <span className={`text-xs ${dep.is_dev ? 'text-text-muted' : 'text-text-secondary'}`}>
                            {dep.is_dev ? 'dev' : 'prod'}
                          </span>
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
            )}
          </div>
        )}

        {/* Active Alerts */}
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            Active Alerts
          </h4>
          {overview.alerts.total === 0 ? (
            <div className="bg-bg-primary rounded-lg border border-border/50 p-4">
              <p className="text-xs text-text-muted">
                No alerts. All dependencies are healthy.
              </p>
            </div>
          ) : (
            <AlertsList onResolve={handleResolveAlert} />
          )}
        </div>

        {/* Cross-Project Packages */}
        {overview.cross_project_packages > 0 && (
          <div>
            <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
              Shared Across Projects
            </h4>
            <div className="flex gap-2 flex-wrap">
              {overview.cross_project_top.map(cp => (
                <div
                  key={`${cp.package_name}-${cp.ecosystem}`}
                  className="bg-bg-tertiary rounded-lg border border-border px-3 py-1.5 flex items-center gap-2"
                >
                  <span className="text-xs font-mono text-text-primary">{cp.package_name}</span>
                  <span className="text-xs text-text-muted">
                    {cp.project_count} projects
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

// ============================================================================
// AlertsList — separate component for lazy alert loading
// ============================================================================

const AlertsList = memo(function AlertsList({ onResolve }: { onResolve: (id: number) => void }) {
  const [alerts, setAlerts] = useState<Array<{
    id: number;
    package_name: string;
    ecosystem: string;
    severity: string;
    title: string;
    alert_type: string;
  }>>([]);

  useEffect(() => {
    cmd('get_dependency_alerts')
      .then(result => setAlerts(result.alerts))
      .catch(() => setAlerts([]));
  }, []);

  if (alerts.length === 0) return null;

  return (
    <div className="space-y-2">
      {alerts.map(alert => (
        <div
          key={alert.id}
          className="flex items-center justify-between bg-bg-primary rounded-lg border border-border/50 px-4 py-3"
        >
          <div className="flex items-center gap-3 min-w-0">
            <SeverityBadge severity={alert.severity} />
            <span className="text-sm text-text-primary truncate">{alert.title}</span>
          </div>
          <div className="flex items-center gap-3 shrink-0 ml-3">
            <span className="text-xs text-text-muted font-mono">{alert.package_name}</span>
            <button
              onClick={() => onResolve(alert.id)}
              className="text-xs text-text-muted hover:text-text-secondary transition-colors"
              title="Dismiss alert"
            >
              dismiss
            </button>
          </div>
        </div>
      ))}
    </div>
  );
});

export default DependencyDashboard;
