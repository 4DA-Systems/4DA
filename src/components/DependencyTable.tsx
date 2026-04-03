import { memo } from 'react';
import { useTranslation } from 'react-i18next';

// ============================================================================
// Types
// ============================================================================

type Severity = 'critical' | 'high' | 'medium' | 'low';

export interface DepEntry {
  name: string;
  version: string | null;
  ecosystem: string;
  is_dev: boolean;
  alerts: Array<{ id: number; severity: string; title: string }>;
}

// ============================================================================
// Constants (shared with DependencyDashboard)
// ============================================================================

export const SEVERITY_COLORS: Record<Severity, string> = {
  critical: 'bg-error/15 text-error border-error/25',
  high: 'bg-[var(--color-accent-action)]/15 text-[var(--color-accent-action)] border-[var(--color-accent-action)]/25',
  medium: 'bg-accent-gold/15 text-accent-gold border-accent-gold/25',
  low: 'bg-white/5 text-text-muted border-border',
};

export const ECOSYSTEM_COLORS: Record<string, string> = {
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
// Shared sub-components
// ============================================================================

export function SeverityBadge({ severity }: { severity: string }) {
  const { t } = useTranslation();
  const sev = (severity as Severity) in SEVERITY_COLORS ? severity as Severity : 'low';
  return (
    <span className={`text-xs px-2 py-0.5 rounded border font-medium ${SEVERITY_COLORS[sev]}`}>
      {t(`deps.severity.${severity}`, severity)}
    </span>
  );
}

export function EcosystemBadge({ ecosystem }: { ecosystem: string }) {
  const color = ECOSYSTEM_COLORS[ecosystem] ?? 'text-text-muted';
  return (
    <span className={`text-xs font-mono ${color}`}>{ecosystem}</span>
  );
}

export function StatCard({ label, value, color }: { label: string; value: number; color?: string }) {
  return (
    <div className="bg-bg-tertiary rounded-lg border border-border px-4 py-3">
      <div className={`text-xl font-semibold ${color ?? 'text-white'}`}>
        {value.toLocaleString()}
      </div>
      <div className="text-xs text-text-muted mt-0.5">{label}</div>
    </div>
  );
}

export function LoadingSkeleton() {
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
// DependencyTable
// ============================================================================

interface DependencyTableProps {
  projectName: string;
  loading: boolean;
  deps: DepEntry[];
}

const DependencyTable = memo(function DependencyTable({ projectName, loading, deps }: DependencyTableProps) {
  const { t } = useTranslation();
  return (
    <div>
      <h4 className="text-xs font-medium text-text-muted uppercase tracking-wider mb-3">
        {projectName} {t('deps.dependencies', 'Dependencies')}
      </h4>
      {loading ? (
        <div className="h-32 bg-bg-tertiary rounded-lg animate-pulse" />
      ) : deps.length === 0 ? (
        <div className="bg-bg-primary rounded-lg border border-border/50 p-4">
          <p className="text-xs text-text-muted">{t('deps.noDeps', 'No dependencies found for this project.')}</p>
        </div>
      ) : (
        <div className="overflow-hidden rounded-lg border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-bg-tertiary text-text-muted text-xs uppercase tracking-wider">
                <th className="text-start px-4 py-2.5 font-medium">Name</th>
                <th className="text-start px-4 py-2.5 font-medium">Version</th>
                <th className="text-start px-4 py-2.5 font-medium">Ecosystem</th>
                <th className="text-start px-4 py-2.5 font-medium">Type</th>
                <th className="text-start px-4 py-2.5 font-medium">Alerts</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {deps.map(dep => (
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
                      <span className="text-xs text-error">{dep.alerts.length}</span>
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
  );
});

export default DependencyTable;
