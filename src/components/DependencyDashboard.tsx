import { useState, useMemo, useEffect, memo } from 'react';
import { cmd } from '../lib/commands';

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
// Data mapping — converts stack health + detected tech into dependency format
// ============================================================================

function healthToFreshness(health: string): Freshness {
  switch (health) {
    case 'healthy': return 'fresh';
    case 'watch': return 'aging';
    case 'warning': return 'stale';
    case 'critical': return 'outdated';
    default: return 'fresh';
  }
}

function trendToSeverity(trend: string, health: string): Severity | null {
  if (health === 'critical' || health === 'warning') {
    return trend === 'declining' ? 'high' : 'medium';
  }
  return null;
}

async function fetchDependencyData(): Promise<Project[]> {
  const [stackHealth, detectedTech] = await Promise.all([
    cmd('get_stack_health'),
    cmd('ace_get_detected_tech'),
  ]);

  // Build a map of detected tech with categories
  const techMap = new Map<string, { category: string; confidence: number }>();
  for (const tech of detectedTech.detected_tech) {
    techMap.set(tech.name.toLowerCase(), { category: tech.category, confidence: tech.confidence });
  }

  // Map stack health technologies to dependencies
  const dependencies: Dependency[] = stackHealth.technologies.map(tech => {
    const detected = techMap.get(tech.name.toLowerCase());
    const freshness = healthToFreshness(tech.health);
    const severity = trendToSeverity(tech.trend, tech.health);

    const alerts: DependencyAlert[] = [];
    if (severity) {
      alerts.push({
        id: `sh-${tech.name}`,
        severity,
        message: `${tech.name} is ${tech.health} with ${tech.trend} trend (${tech.signal_count} signals)`,
        dependency: tech.name,
      });
    }

    return {
      name: tech.name,
      version: '',
      ecosystem: detected?.category ?? 'detected',
      freshness,
      alerts,
    };
  });

  // Add any detected tech not in stack health
  for (const tech of detectedTech.detected_tech) {
    const exists = dependencies.some(d => d.name.toLowerCase() === tech.name.toLowerCase());
    if (!exists) {
      dependencies.push({
        name: tech.name,
        version: '',
        ecosystem: tech.category,
        freshness: tech.confidence >= 0.8 ? 'fresh' : 'aging',
        alerts: [],
      });
    }
  }

  if (dependencies.length === 0) {
    return [];
  }

  return [{
    name: 'Your Stack',
    path: 'detected',
    dependencies,
  }];
}

// ============================================================================
// DependencyDashboard
// ============================================================================

const DependencyDashboard = memo(function DependencyDashboard() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProject, setSelectedProject] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  useEffect(() => {
    setLoading(true);
    fetchDependencyData()
      .then(data => {
        setProjects(data);
        setSelectedProject(0);
        setError(false);
      })
      .catch(() => {
        setProjects([]);
        setError(true);
      })
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <LoadingSkeleton />;

  if (error || projects.length === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border">
          <h3 className="text-sm font-medium text-white">Dependency Health</h3>
          <p className="text-xs text-text-muted mt-1">
            Monitor freshness and vulnerabilities across your projects.
          </p>
        </div>
        <div className="p-5">
          <div className="bg-bg-primary rounded-lg border border-border/50 p-6 text-center">
            <p className="text-sm text-text-muted mb-2">No dependency data available</p>
            <p className="text-xs text-text-muted/60 leading-relaxed">
              Run a context scan to detect your tech stack and dependencies.
              4DA will track their health automatically.
            </p>
          </div>
        </div>
      </div>
    );
  }

  const project = projects[selectedProject] ?? projects[0];
  const deps = project.dependencies;

  const stats = {
    total: deps.length,
    fresh: deps.filter(d => d.freshness === 'fresh').length,
    stale: deps.filter(d => d.freshness === 'stale' || d.freshness === 'outdated').length,
    vulnerable: deps.filter(d => d.alerts.length > 0).length,
  };

  const allAlerts = deps.flatMap(d => d.alerts);

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
        {projects.length > 1 && (
          <select
            value={selectedProject}
            onChange={e => setSelectedProject(Number(e.target.value))}
            className="bg-bg-tertiary border border-border rounded-lg px-3 py-1.5 text-sm text-text-primary focus:outline-none focus:border-text-muted/50"
          >
            {projects.map((p, i) => (
              <option key={p.path} value={i}>{p.name}</option>
            ))}
          </select>
        )}
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
                  <td className="px-4 py-2.5 text-text-secondary font-mono">{dep.version || '--'}</td>
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
