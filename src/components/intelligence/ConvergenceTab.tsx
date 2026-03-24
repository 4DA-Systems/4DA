// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type {
  TechConvergenceReport,
  ProjectHealthComparison,
  CrossProjectDep,
} from '../../lib/commands';

// ============================================================================
// Sub-components
// ============================================================================

function LoadingSkeleton() {
  return (
    <div className="p-5 space-y-4">
      <div className="grid grid-cols-3 gap-3">
        {[1, 2, 3].map(i => (
          <div key={i} className="h-16 bg-bg-tertiary rounded-lg animate-pulse" />
        ))}
      </div>
      <div className="h-48 bg-bg-tertiary rounded-lg animate-pulse" />
    </div>
  );
}

function StatCard({ label, value, suffix, color }: { label: string; value: string | number; suffix?: string; color?: string }) {
  return (
    <div className="bg-bg-tertiary rounded-lg border border-border px-4 py-3">
      <div className={`text-xl font-semibold ${color ?? 'text-white'}`}>
        {value}{suffix && <span className="text-sm ms-0.5">{suffix}</span>}
      </div>
      <div className="text-xs text-text-muted mt-0.5">{label}</div>
    </div>
  );
}

const BUS_FACTOR_COLORS: Record<string, string> = {
  critical: 'bg-error/15 text-error border-error/25',
  high: 'bg-[#F97316]/15 text-[#F97316] border-[#F97316]/25',
  medium: 'bg-accent-gold/15 text-accent-gold border-accent-gold/25',
  low: 'bg-white/5 text-text-muted border-border',
};

// ============================================================================
// ConvergenceTab
// ============================================================================

export const ConvergenceTab = memo(function ConvergenceTab() {
  const { t } = useTranslation();
  const [convergence, setConvergence] = useState<TechConvergenceReport | null>(null);
  const [health, setHealth] = useState<ProjectHealthComparison | null>(null);
  const [crossDeps, setCrossDeps] = useState<CrossProjectDep[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);

    Promise.allSettled([
      cmd('get_tech_convergence'),
      cmd('get_project_health_comparison'),
      cmd('get_cross_project_dependencies'),
    ])
      .then(([convResult, healthResult, depsResult]) => {
        if (convResult.status === 'fulfilled') setConvergence(convResult.value);
        if (healthResult.status === 'fulfilled') setHealth(healthResult.value);
        if (depsResult.status === 'fulfilled') setCrossDeps(depsResult.value);
      })
      .catch(() => setError('Failed to load convergence data.'))
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <LoadingSkeleton />;

  const hasData = convergence || health || crossDeps.length > 0;

  if (error && !hasData) {
    return (
      <div className="p-5">
        <div className="bg-error/10 rounded-lg border border-error/20 p-4">
          <p className="text-xs text-error">{error}</p>
        </div>
      </div>
    );
  }

  if (!hasData) {
    return (
      <div className="p-5">
        <div className="bg-bg-primary rounded-lg border border-border/50 p-6 text-center">
          <p className="text-sm text-text-muted mb-2">{t('convergence.noData')}</p>
          <p className="text-xs text-text-muted/60 leading-relaxed">
            {t('convergence.noDataDesc')}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-5 space-y-5 overflow-y-auto">
      {/* Summary Stats */}
      {convergence && (
        <div className="grid grid-cols-3 gap-3">
          <StatCard label={t('convergence.projects')} value={convergence.total_projects} />
          <StatCard label={t('convergence.sharedTech')} value={convergence.shared_technologies.length} color="text-success" />
          <StatCard
            label={t('convergence.convergence')}
            value={(convergence.convergence_score * 100).toFixed(0)}
            suffix="%"
            color="text-accent-gold"
          />
        </div>
      )}

      {/* Shared Technologies */}
      {convergence && convergence.shared_technologies.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('convergence.sharedTechnologies')}
          </h4>
          <div className="space-y-2">
            {convergence.shared_technologies.map(tech => (
              <div key={tech.name} className="flex items-center gap-3 bg-bg-primary rounded-lg border border-border/50 px-4 py-2.5">
                <span className="text-sm text-text-primary font-mono flex-1 min-w-0 truncate">{tech.name}</span>
                <span className="text-xs text-text-muted shrink-0">{tech.category}</span>
                <span className="text-xs text-text-secondary shrink-0">
                  {t('convergence.projectCount', { count: tech.project_count })}
                </span>
                <div className="w-12 h-1.5 bg-bg-tertiary rounded-full overflow-hidden shrink-0">
                  <div
                    className="h-full bg-success rounded-full"
                    style={{ width: `${Math.min(tech.adoption_pct * 100, 100)}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Unique Technologies (Bus Factor Risks) */}
      {convergence && convergence.unique_technologies.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('convergence.uniqueToSingleProjects')}
          </h4>
          <div className="flex gap-2 flex-wrap">
            {convergence.unique_technologies.map(tech => {
              const riskClass = BUS_FACTOR_COLORS[tech.bus_factor_risk] ?? BUS_FACTOR_COLORS.low;
              return (
                <div
                  key={`${tech.name}-${tech.project_path}`}
                  className={`rounded-lg border px-3 py-1.5 flex items-center gap-2 ${riskClass}`}
                >
                  <span className="text-xs font-mono">{tech.name}</span>
                  <span className="text-xs opacity-60">{tech.category}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Cross-Project Dependencies */}
      {crossDeps.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('convergence.crossProjectDeps')}
          </h4>
          <div className="overflow-hidden rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                  <th className="text-start px-4 py-2.5 font-medium">{t('convergence.thPackage')}</th>
                  <th className="text-start px-4 py-2.5 font-medium">{t('convergence.thEcosystem')}</th>
                  <th className="text-start px-4 py-2.5 font-medium">{t('convergence.thProjects')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {crossDeps.slice(0, 15).map(dep => (
                  <tr key={`${dep.name}-${dep.ecosystem}`} className="hover:bg-[#1A1A1A] transition-colors">
                    <td className="px-4 py-2.5 font-mono text-text-primary">{dep.name}</td>
                    <td className="px-4 py-2.5 text-text-muted text-xs">{dep.ecosystem}</td>
                    <td className="px-4 py-2.5 text-text-secondary">{dep.project_count}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Project Health Comparison */}
      {health && health.projects.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            {t('convergence.projectHealth')}
          </h4>
          <div className="overflow-hidden rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                  <th className="text-start px-4 py-2.5 font-medium">{t('convergence.thProject')}</th>
                  <th className="text-start px-4 py-2.5 font-medium">{t('convergence.thDeps')}</th>
                  <th className="text-start px-4 py-2.5 font-medium">{t('convergence.thFreshness')}</th>
                  <th className="text-start px-4 py-2.5 font-medium">{t('convergence.thVulns')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {health.projects.map(proj => (
                  <tr key={proj.project_path} className="hover:bg-[#1A1A1A] transition-colors">
                    <td className="px-4 py-2.5 text-text-primary truncate max-w-[200px]">{proj.project_name}</td>
                    <td className="px-4 py-2.5 text-text-secondary">{proj.dependency_count}</td>
                    <td className="px-4 py-2.5">
                      <span className={`text-xs font-medium ${
                        proj.freshness_score >= 0.8 ? 'text-success' :
                        proj.freshness_score >= 0.5 ? 'text-accent-gold' : 'text-error'
                      }`}>
                        {(proj.freshness_score * 100).toFixed(0)}%
                      </span>
                    </td>
                    <td className="px-4 py-2.5">
                      <span className={`text-xs font-medium ${proj.vulnerability_count > 0 ? 'text-error' : 'text-success'}`}>
                        {proj.vulnerability_count}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
});
