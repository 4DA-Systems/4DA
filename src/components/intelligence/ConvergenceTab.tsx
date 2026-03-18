// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, memo } from 'react';
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
        {value}{suffix && <span className="text-sm ml-0.5">{suffix}</span>}
      </div>
      <div className="text-xs text-text-muted mt-0.5">{label}</div>
    </div>
  );
}

const BUS_FACTOR_COLORS: Record<string, string> = {
  critical: 'bg-[#EF4444]/15 text-[#EF4444] border-[#EF4444]/25',
  high: 'bg-[#F97316]/15 text-[#F97316] border-[#F97316]/25',
  medium: 'bg-[#D4AF37]/15 text-[#D4AF37] border-[#D4AF37]/25',
  low: 'bg-white/5 text-text-muted border-border',
};

// ============================================================================
// ConvergenceTab
// ============================================================================

export const ConvergenceTab = memo(function ConvergenceTab() {
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
          <p className="text-sm text-text-muted mb-2">No project data available</p>
          <p className="text-xs text-text-muted/60 leading-relaxed">
            Run ACE discovery to analyze your projects. 4DA will detect shared
            technologies, cross-project dependencies, and convergence patterns.
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
          <StatCard label="Projects" value={convergence.total_projects} />
          <StatCard label="Shared Tech" value={convergence.shared_technologies.length} color="text-[#22C55E]" />
          <StatCard
            label="Convergence"
            value={(convergence.convergence_score * 100).toFixed(0)}
            suffix="%"
            color="text-[#D4AF37]"
          />
        </div>
      )}

      {/* Shared Technologies */}
      {convergence && convergence.shared_technologies.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
            Shared Technologies
          </h4>
          <div className="space-y-2">
            {convergence.shared_technologies.map(tech => (
              <div key={tech.name} className="flex items-center gap-3 bg-bg-primary rounded-lg border border-border/50 px-4 py-2.5">
                <span className="text-sm text-text-primary font-mono flex-1 min-w-0 truncate">{tech.name}</span>
                <span className="text-xs text-text-muted shrink-0">{tech.category}</span>
                <span className="text-xs text-text-secondary shrink-0">
                  {tech.project_count} project{tech.project_count !== 1 ? 's' : ''}
                </span>
                <div className="w-12 h-1.5 bg-bg-tertiary rounded-full overflow-hidden shrink-0">
                  <div
                    className="h-full bg-[#22C55E] rounded-full"
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
            Unique to Single Projects
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
            Cross-Project Dependencies
          </h4>
          <div className="overflow-hidden rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                  <th className="text-left px-4 py-2.5 font-medium">Package</th>
                  <th className="text-left px-4 py-2.5 font-medium">Ecosystem</th>
                  <th className="text-left px-4 py-2.5 font-medium">Projects</th>
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
            Project Health
          </h4>
          <div className="overflow-hidden rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                  <th className="text-left px-4 py-2.5 font-medium">Project</th>
                  <th className="text-left px-4 py-2.5 font-medium">Deps</th>
                  <th className="text-left px-4 py-2.5 font-medium">Freshness</th>
                  <th className="text-left px-4 py-2.5 font-medium">Vulns</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {health.projects.map(proj => (
                  <tr key={proj.project_path} className="hover:bg-[#1A1A1A] transition-colors">
                    <td className="px-4 py-2.5 text-text-primary truncate max-w-[200px]">{proj.project_name}</td>
                    <td className="px-4 py-2.5 text-text-secondary">{proj.dependency_count}</td>
                    <td className="px-4 py-2.5">
                      <span className={`text-xs font-medium ${
                        proj.freshness_score >= 0.8 ? 'text-[#22C55E]' :
                        proj.freshness_score >= 0.5 ? 'text-[#D4AF37]' : 'text-[#EF4444]'
                      }`}>
                        {(proj.freshness_score * 100).toFixed(0)}%
                      </span>
                    </td>
                    <td className="px-4 py-2.5">
                      <span className={`text-xs font-medium ${proj.vulnerability_count > 0 ? 'text-[#EF4444]' : 'text-[#22C55E]'}`}>
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
