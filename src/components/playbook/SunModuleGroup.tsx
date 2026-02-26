import { useState, useCallback } from 'react';
import { SunSparkline } from './SunSparkline';
import type { SunStatus } from '../../store/suns-slice';
import type { ModuleHealth } from '../../store/suns-slice';

const MODULE_COLORS: Record<string, string> = {
  S: '#D4AF37',
  T: '#3B82F6',
  R: '#22C55E',
  E1: '#F97316',
  E2: '#8B5CF6',
  T2: '#06B6D4',
  S2: '#EC4899',
};

function formatInterval(secs: number): string {
  if (secs >= 604800) return `${Math.round(secs / 604800)}w`;
  if (secs >= 86400) return `${Math.round(secs / 86400)}d`;
  if (secs >= 3600) return `${Math.round(secs / 3600)}h`;
  if (secs >= 60) return `${Math.round(secs / 60)}m`;
  return `${secs}s`;
}

function timeAgo(isoStr: string | null): string {
  if (!isoStr) return 'Never';
  const diff = (Date.now() - new Date(isoStr).getTime()) / 1000;
  if (diff < 60) return 'Just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

interface SunModuleGroupProps {
  moduleId: string;
  moduleName: string;
  moduleHealth: ModuleHealth | undefined;
  suns: SunStatus[];
  onToggle: (id: string, enabled: boolean) => void;
  onTrigger: (id: string) => void;
}

export function SunModuleGroup({
  moduleId,
  moduleName,
  moduleHealth,
  suns,
  onToggle,
  onTrigger,
}: SunModuleGroupProps) {
  const color = MODULE_COLORS[moduleId] || '#A0A0A0';
  const healthPct = moduleHealth ? Math.round(moduleHealth.score * 100) : 0;

  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{ border: '1px solid #2A2A2A' }}
    >
      {/* Module header */}
      <div
        className="flex items-center justify-between px-3 py-2"
        style={{ background: `${color}08` }}
      >
        <div className="flex items-center gap-2">
          <span
            className="w-6 h-6 flex items-center justify-center rounded text-xs font-bold"
            style={{ background: `${color}20`, color }}
          >
            {moduleId}
          </span>
          <span className="text-sm font-medium text-white">{moduleName}</span>
          <span className="text-xs" style={{ color: '#666666' }}>
            {suns.length} sun{suns.length !== 1 ? 's' : ''}
          </span>
        </div>
        {moduleHealth && (
          <div className="flex items-center gap-2">
            {moduleHealth.lessons_completed > 0 && (
              <span className="text-xs" style={{ color: '#666666' }}>
                {moduleHealth.lessons_completed}/{moduleHealth.total_lessons}{' '}
                lessons
              </span>
            )}
            <span
              className="text-xs font-medium px-1.5 py-0.5 rounded"
              style={{
                color:
                  healthPct >= 70
                    ? '#22C55E'
                    : healthPct >= 40
                      ? '#D4AF37'
                      : '#EF4444',
                background:
                  healthPct >= 70
                    ? '#22C55E15'
                    : healthPct >= 40
                      ? '#D4AF3715'
                      : '#EF444415',
              }}
            >
              {healthPct}%
            </span>
          </div>
        )}
      </div>

      {/* Sun rows */}
      <div className="divide-y" style={{ borderColor: '#1F1F1F' }}>
        {suns.map((sun) => (
          <SunRowCompact
            key={sun.id}
            sun={sun}
            onToggle={onToggle}
            onTrigger={onTrigger}
          />
        ))}
      </div>
    </div>
  );
}

function SunRowCompact({
  sun,
  onToggle,
  onTrigger,
}: {
  sun: SunStatus;
  onToggle: (id: string, enabled: boolean) => void;
  onTrigger: (id: string) => void;
}) {
  const [triggering, setTriggering] = useState(false);

  const handleTrigger = useCallback(() => {
    setTriggering(true);
    onTrigger(sun.id);
    setTimeout(() => setTriggering(false), 1500);
  }, [sun.id, onTrigger]);

  return (
    <div
      className="flex items-center gap-2 px-3 py-2"
      style={{ background: '#141414' }}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span
            className="text-sm"
            style={{ color: sun.enabled ? '#FFFFFF' : '#666666' }}
          >
            {sun.name}
          </span>
          <span className="text-xs" style={{ color: '#666666' }}>
            {formatInterval(sun.interval_secs)}
          </span>
        </div>
        <div className="flex items-center gap-2 mt-0.5">
          <span className="text-xs" style={{ color: '#666666' }}>
            {timeAgo(sun.last_run)}
          </span>
          {sun.run_count > 0 && (
            <span className="text-xs" style={{ color: '#666666' }}>
              {sun.run_count} runs
            </span>
          )}
        </div>
      </div>

      <SunSparkline sunId={sun.id} />

      <button
        onClick={handleTrigger}
        disabled={triggering || !sun.enabled}
        className="text-xs px-2 py-0.5 rounded transition-colors"
        style={{
          border: '1px solid #2A2A2A',
          color: triggering ? '#D4AF37' : '#A0A0A0',
          opacity: sun.enabled ? 1 : 0.5,
        }}
      >
        {triggering ? '...' : 'Run'}
      </button>

      <button
        onClick={() => onToggle(sun.id, !sun.enabled)}
        className="w-8 h-4 rounded-full relative flex-shrink-0"
        style={{ background: sun.enabled ? '#22C55E' : '#2A2A2A' }}
      >
        <span
          className="absolute top-0.5 w-3 h-3 rounded-full transition-transform"
          style={{
            background: '#FFFFFF',
            left: sun.enabled ? '16px' : '2px',
          }}
        />
      </button>
    </div>
  );
}
