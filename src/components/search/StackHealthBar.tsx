import { useTranslation } from 'react-i18next';

export interface TechHealthEntry {
  name: string;
  category: string;
  status: string;
  signal_count_7d: number;
  days_since_engagement: number;
  has_knowledge_gap: boolean;
}

export interface MissedIntelligence {
  total_count: number;
  critical_count: number;
  high_count: number;
  example_titles: string[];
}

export interface StackHealth {
  technologies: TechHealthEntry[];
  stack_score: number;
  signals_this_week: number;
  suggested_queries: string[];
  missed_signals: MissedIntelligence;
}

interface StackHealthBarProps {
  health: StackHealth | null;
  onSuggestedQuery: (query: string) => void;
}

const statusColors: Record<string, string> = {
  healthy: '#22C55E',
  attention: '#EAB308',
  stale: '#666666',
  critical: '#EF4444',
};

const statusIcons: Record<string, string> = {
  healthy: '\u25CF',    // filled circle
  attention: '\u26A0',  // warning
  stale: '\u25CC',      // dotted circle
  critical: '\u25BC',   // down triangle
};

export function StackHealthBar({ health, onSuggestedQuery }: StackHealthBarProps) {
  const { t } = useTranslation();

  if (!health) return null;

  return (
    <div className="space-y-3">
      {/* Tech pills row */}
      <div className="flex items-center gap-2 flex-wrap">
        {health.technologies.map((tech) => (
          <span
            key={tech.name}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-full bg-bg-secondary border border-border"
            title={`${tech.name}: ${tech.status} (${tech.signal_count_7d} signals this week)`}
          >
            <span className="text-gray-300">{tech.name}</span>
            <span style={{ color: statusColors[tech.status] || '#666666' }}>
              {statusIcons[tech.status] || '\u25CF'}
            </span>
          </span>
        ))}

        {/* Stack score badge */}
        <span className="ml-auto inline-flex items-center px-2.5 py-1 text-xs rounded-full bg-cyan-500/10 border border-cyan-500/20 text-cyan-400 font-medium">
          {health.stack_score}/100
        </span>
      </div>

      {/* Missed signals banner */}
      {health.missed_signals.total_count > 0 && (
        <div className="flex items-center gap-2 px-3 py-2 bg-yellow-500/10 border border-yellow-500/20 rounded-lg">
          <span className="text-yellow-400 text-xs">{'\u26A0'}</span>
          <span className="text-xs text-yellow-300">
            {t('search.missedSignals', { count: health.missed_signals.total_count, critical: health.missed_signals.critical_count })}
          </span>
        </div>
      )}

      {/* Suggested queries */}
      {health.suggested_queries.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {health.suggested_queries.map((sq) => (
            <button
              key={sq}
              onClick={() => onSuggestedQuery(sq)}
              className="px-2.5 py-1 text-xs bg-bg-secondary rounded-md border border-border text-gray-400 hover:text-cyan-400 hover:border-cyan-500/30 transition-all"
            >
              {sq}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
