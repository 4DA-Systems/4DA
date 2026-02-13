import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ProjectHealth } from '../../types';

function ScoreBar({ label, score, color }: { label: string; score: number; color: string }) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-[11px] text-gray-400 w-20">{label}</span>
      <div className="flex-1 h-2.5 bg-[#1F1F1F] rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all ${color}`}
          style={{ width: `${Math.round(score * 100)}%` }}
        />
      </div>
      <span className="text-[10px] text-gray-500 w-8 text-right">{Math.round(score * 100)}</span>
    </div>
  );
}

export function ProjectHealthRadar() {
  const [projects, setProjects] = useState<ProjectHealth[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      setLoading(true);
      try {
        const result = await invoke<ProjectHealth[]>('get_project_health');
        setProjects(result);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 bg-teal-500/20 rounded-lg flex items-center justify-center">
          <span>📡</span>
        </div>
        <div>
          <h3 className="text-sm font-medium text-white">Project Health Radar</h3>
          <p className="text-xs text-gray-500">Dependency freshness, security, momentum, community</p>
        </div>
      </div>

      {loading && (
        <div className="flex items-center gap-2 py-4 justify-center">
          <div className="w-4 h-4 border-2 border-teal-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-xs text-gray-500">Scanning projects...</span>
        </div>
      )}

      {error && (
        <div className="text-xs text-red-400 p-3 bg-red-500/10 rounded border border-red-500/20">{error}</div>
      )}

      {!loading && projects.length === 0 && !error && (
        <p className="text-xs text-gray-500 text-center py-4">
          No projects detected. Add context directories in settings to enable health tracking.
        </p>
      )}

      {projects.map((p) => (
        <div key={p.project_path} className="p-4 bg-[#0A0A0A] rounded-lg border border-[#2A2A2A]">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm text-white font-medium">{p.project_name}</span>
            <span className={`text-xs px-2 py-0.5 rounded ${
              p.overall_score >= 0.7 ? 'bg-green-500/20 text-green-400' :
              p.overall_score >= 0.4 ? 'bg-amber-500/20 text-amber-400' :
              'bg-red-500/20 text-red-400'
            }`}>
              {Math.round(p.overall_score * 100)} / 100
            </span>
          </div>

          <div className="space-y-2">
            <ScoreBar label="Freshness" score={p.freshness.score} color="bg-green-500/70" />
            <ScoreBar label="Security" score={p.security.score} color="bg-red-500/70" />
            <ScoreBar label="Momentum" score={p.momentum.score} color="bg-blue-500/70" />
            <ScoreBar label="Community" score={p.community.score} color="bg-purple-500/70" />
          </div>

          {/* Details on hover/click */}
          <div className="mt-3 grid grid-cols-2 gap-2 text-[10px] text-gray-500">
            <span title={p.freshness.details}>{p.freshness.label}</span>
            <span title={p.security.details}>{p.security.label}</span>
            <span title={p.momentum.details}>{p.momentum.label}</span>
            <span title={p.community.details}>{p.community.label}</span>
          </div>

          {/* Alerts */}
          {p.alerts.length > 0 && (
            <div className="mt-3 space-y-1">
              {p.alerts.slice(0, 3).map((alert, i) => (
                <div key={i} className="flex items-center gap-2 text-[11px]">
                  <span className={`w-1.5 h-1.5 rounded-full ${
                    alert.severity === 'critical' ? 'bg-red-400' :
                    alert.severity === 'high' ? 'bg-amber-400' : 'bg-gray-400'
                  }`} />
                  <span className="text-gray-400">{alert.message}</span>
                </div>
              ))}
            </div>
          )}

          <div className="mt-2 text-[10px] text-gray-600">
            {p.dependency_count} dependencies · Last checked {p.last_checked ? new Date(p.last_checked).toLocaleDateString() : 'never'}
          </div>
        </div>
      ))}
    </div>
  );
}
