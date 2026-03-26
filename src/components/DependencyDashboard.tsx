import { useState, useEffect, useMemo, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import DependencyTable, {
  StatCard,
  EcosystemBadge,
  LoadingSkeleton,
} from './DependencyTable';
import VulnerabilitySummary from './VulnerabilitySummary';
import type { DepEntry } from './DependencyTable';

// ============================================================================
// Types
// ============================================================================

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

interface AceScanSummary {
  projects_scanned: number;
  total_dependencies: number;
  primary_stack: string;
  languages: string[];
  frameworks: string[];
  has_data: boolean;
}

// ============================================================================
// DependencyDashboard
// ============================================================================

const DependencyDashboard = memo(function DependencyDashboard() {
  const { t } = useTranslation();
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
          setSelectedProject(data.projects[0]!.path);
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
        .catch((e) => console.debug('[DependencyDashboard] scan summary:', e));
    }
  }, [loading, overview]);

  if (loading) return <LoadingSkeleton />;

  if (!overview || overview.total_dependencies === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border">
          <h3 className="text-sm font-medium text-white">{t('deps.title')}</h3>
          <p className="text-xs text-text-muted mt-1">
            {t('deps.trackDesc')}
          </p>
        </div>
        <div className="p-5">
          {scanSummary && scanSummary.has_data ? (
            <div className="bg-bg-primary rounded-lg border border-border/50 p-6 text-center">
              <div className="flex items-center justify-center gap-2 mb-2">
                <div className="w-2 h-2 rounded-full bg-accent-gold/60 animate-pulse" />
                <p className="text-sm text-text-secondary">
                  {t('deps.detected', { stack: scanSummary.primary_stack || scanSummary.languages.join(', ') })}
                </p>
              </div>
              <p className="text-xs text-text-muted mb-4">
                {t('deps.depCountAcrossProjects', { deps: scanSummary.total_dependencies, count: scanSummary.projects_scanned })}
              </p>
              <p className="text-xs text-text-muted/60 leading-relaxed">
                {t('deps.runScanDesc')}
              </p>
            </div>
          ) : (
            <div className="bg-bg-primary rounded-lg border border-border/50 p-6 text-center">
              <p className="text-sm text-text-muted mb-2">{t('deps.noData')}</p>
              <p className="text-xs text-text-muted/60 leading-relaxed">
                {t('deps.noDataDesc')}
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
          <h3 className="text-sm font-medium text-white">{t('deps.title')}</h3>
          <p className="text-xs text-text-muted mt-1">
            {t('deps.summary', { projects: overview.total_projects, deps: overview.total_dependencies })}
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
          <StatCard label={t('deps.total')} value={overview.total_dependencies} />
          <StatCard label={t('deps.direct')} value={overview.direct_dependencies} color="text-success" />
          <StatCard label={t('deps.dev')} value={overview.dev_dependencies} color="text-accent-gold" />
          <StatCard
            label={t('deps.alerts')}
            value={overview.alerts.total}
            color={overview.alerts.total > 0 ? 'text-error' : 'text-success'}
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
          <DependencyTable
            projectName={activeProject.name}
            loading={loadingProject}
            deps={projectDeps}
          />
        )}

        {/* Vulnerability Summary: Alerts + Cross-Project */}
        <VulnerabilitySummary
          alertTotal={overview.alerts.total}
          crossProjectPackages={overview.cross_project_packages}
          crossProjectTop={overview.cross_project_top}
          onResolveAlert={handleResolveAlert}
        />
      </div>
    </div>
  );
});

export default DependencyDashboard;
