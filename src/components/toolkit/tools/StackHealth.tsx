import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

// --- Types ---

interface ProjectHealthEntry {
  name: string;
  path: string;
  scores: {
    freshness: number;
    security: number;
    momentum: number;
    community: number;
  };
  alerts: Array<{ severity: string; message: string }>;
  dependency_count: number;
  overall: number;
}

interface ProjectHealth {
  projects: ProjectHealthEntry[];
  generated_at: string;
}

// --- Helpers ---

const DIMENSIONS = ['freshness', 'security', 'momentum', 'community'] as const;
type Dimension = (typeof DIMENSIONS)[number];

const DIMENSION_ICONS: Record<Dimension, string> = {
  freshness: 'M12 8v4l3 3',       // clock-like
  security: 'M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z', // shield
  momentum: 'M13 2L3 14h9l-1 8 10-12h-9l1-8z',              // lightning
  community: 'M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2',     // users
};

function scoreColor(score: number): string {
  if (score > 0.7) return '#22C55E';
  if (score > 0.4) return '#D4AF37';
  return '#EF4444';
}

function severityColor(severity: string): string {
  switch (severity.toLowerCase()) {
    case 'critical': return '#EF4444';
    case 'warning': return '#D4AF37';
    default: return '#A0A0A0';
  }
}

function formatScore(score: number): string {
  return Math.round(score * 100).toString();
}

// --- Component ---

export default function StackHealth() {
  const [data, setData] = useState<ProjectHealth | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const fetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<ProjectHealth>('get_project_health');
      setData(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetch();
  }, [fetch]);

  const toggleExpand = (name: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const totalAlerts = data?.projects.reduce((sum, p) => sum + p.alerts.length, 0) ?? 0;

  return (
    <div className="space-y-4">
      {/* Controls */}
      <div className="flex items-center gap-3 flex-wrap">
        <button
          onClick={fetch}
          disabled={loading}
          className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {loading ? (
            <>
              <div className="w-3.5 h-3.5 border-2 border-[#0A0A0A]/30 border-t-[#0A0A0A] rounded-full animate-spin" />
              Scanning...
            </>
          ) : (
            <>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M23 4v6h-6" />
                <path d="M1 20v-6h6" />
                <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
              </svg>
              Refresh
            </>
          )}
        </button>

        {data && (
          <div className="flex items-center gap-4 text-xs text-[#A0A0A0]">
            <span>
              {data.projects.length} project{data.projects.length !== 1 ? 's' : ''}
            </span>
            {totalAlerts > 0 && (
              <span className="text-[#EF4444]">
                {totalAlerts} alert{totalAlerts !== 1 ? 's' : ''}
              </span>
            )}
            <span className="text-[#666] font-mono">{data.generated_at}</span>
          </div>
        )}
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center gap-3 px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#EF4444" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <span className="text-sm text-[#EF4444] flex-1">{error}</span>
          <button
            onClick={fetch}
            className="text-xs text-[#EF4444] hover:text-white transition-colors underline"
          >
            Retry
          </button>
        </div>
      )}

      {/* Loading state */}
      {loading && !data && (
        <div className="flex flex-col items-center justify-center py-16 text-[#666]">
          <div className="w-8 h-8 border-2 border-[#2A2A2A] border-t-white rounded-full animate-spin mb-4" />
          <p className="text-sm">Analyzing project health...</p>
        </div>
      )}

      {/* Empty state */}
      {!loading && !data && !error && (
        <div className="flex flex-col items-center justify-center py-16 text-[#666]">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mb-3 opacity-50">
            <path d="M22 12h-4l-3 9L9 3l-3 9H2" />
          </svg>
          <p className="text-sm">No health data yet</p>
          <p className="text-xs mt-1">Click Refresh to scan project dependencies</p>
        </div>
      )}

      {/* Project cards */}
      {data && data.projects.length === 0 && (
        <div className="text-center py-10 text-[#666] text-sm">
          No projects detected. ACE must discover projects first.
        </div>
      )}

      {data?.projects.map((project) => {
        const isOpen = expanded.has(project.name);
        const criticalCount = project.alerts.filter((a) => a.severity.toLowerCase() === 'critical').length;

        return (
          <div
            key={project.name}
            className="bg-[#141414] border border-[#2A2A2A] rounded-xl overflow-hidden"
          >
            {/* Card header */}
            <button
              onClick={() => toggleExpand(project.name)}
              className="w-full flex items-center gap-3 px-4 py-3 hover:bg-[#1F1F1F] transition-colors text-left"
            >
              <svg
                width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"
                className={`text-[#666] transition-transform shrink-0 ${isOpen ? 'rotate-90' : ''}`}
              >
                <path d="M9 18l6-6-6-6" />
              </svg>
              <span className="text-sm text-white font-medium truncate flex-1">{project.name}</span>

              {criticalCount > 0 && (
                <span className="px-1.5 py-0.5 text-[10px] font-medium bg-[#EF4444]/15 text-[#EF4444] rounded">
                  {criticalCount} critical
                </span>
              )}

              <span className="text-xs text-[#666] font-mono shrink-0">
                {project.dependency_count} deps
              </span>

              {/* Overall score pill */}
              <span
                className="px-2 py-0.5 text-xs font-mono font-semibold rounded shrink-0"
                style={{
                  color: scoreColor(project.overall),
                  backgroundColor: `${scoreColor(project.overall)}15`,
                }}
              >
                {formatScore(project.overall)}
              </span>
            </button>

            {/* Expanded content */}
            {isOpen && (
              <div className="px-4 pb-4 border-t border-[#2A2A2A]">
                {/* Score bars */}
                <div className="grid grid-cols-2 gap-3 mt-3">
                  {DIMENSIONS.map((dim) => {
                    const score = project.scores[dim];
                    const color = scoreColor(score);
                    return (
                      <div key={dim} className="flex items-center gap-2">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0">
                          <path d={DIMENSION_ICONS[dim]} />
                        </svg>
                        <span className="text-xs text-[#A0A0A0] capitalize w-20 shrink-0">{dim}</span>
                        <div className="flex-1 h-1.5 bg-[#1F1F1F] rounded-full overflow-hidden">
                          <div
                            className="h-full rounded-full transition-all duration-500"
                            style={{ width: `${score * 100}%`, backgroundColor: color }}
                          />
                        </div>
                        <span className="text-xs font-mono w-8 text-right" style={{ color }}>
                          {formatScore(score)}
                        </span>
                      </div>
                    );
                  })}
                </div>

                {/* Path */}
                <div className="mt-3 text-[10px] font-mono text-[#666] truncate" title={project.path}>
                  {project.path}
                </div>

                {/* Alerts */}
                {project.alerts.length > 0 && (
                  <div className="mt-3 space-y-1">
                    {project.alerts.map((alert, i) => (
                      <div
                        key={i}
                        className="flex items-start gap-2 px-3 py-2 rounded-lg text-xs"
                        style={{
                          backgroundColor: `${severityColor(alert.severity)}10`,
                          borderLeft: `2px solid ${severityColor(alert.severity)}`,
                        }}
                      >
                        <span
                          className="uppercase text-[10px] font-semibold shrink-0 mt-px"
                          style={{ color: severityColor(alert.severity) }}
                        >
                          {alert.severity}
                        </span>
                        <span className="text-[#A0A0A0]">{alert.message}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
