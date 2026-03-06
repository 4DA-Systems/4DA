import type { Recommendation } from '../../types/calibration';
import { priorityColor } from './calibration-utils';

export function DimensionBar({ label, score }: { label: string; score: number }) {
  const pct = Math.round((score / 25) * 100);
  const color = pct >= 80 ? '#22C55E' : pct >= 50 ? '#D4AF37' : pct >= 25 ? '#F59E0B' : '#EF4444';
  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 2 }}>
        <span style={{ fontSize: 11, color: '#A0A0A0' }}>{label}</span>
        <span style={{ fontSize: 11, color, fontFamily: 'JetBrains Mono, monospace' }}>{score}/25</span>
      </div>
      <div style={{ height: 4, background: '#2A2A2A', borderRadius: 2, overflow: 'hidden' }}>
        <div style={{ height: '100%', width: `${pct}%`, background: color, borderRadius: 2, transition: 'width 0.3s ease' }} />
      </div>
    </div>
  );
}

export function StatusRow({ label, ok, detail }: { label: string; ok: boolean; detail: string }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <span style={{ color: ok ? '#22C55E' : '#EF4444', fontSize: 14 }}>
        {ok ? '\u2713' : '\u2717'}
      </span>
      <span style={{ fontSize: 12, color: '#A0A0A0' }}>{label}:</span>
      <span style={{ fontSize: 12, color: '#FFFFFF', fontFamily: 'JetBrains Mono, monospace' }}>
        {detail}
      </span>
    </div>
  );
}

interface ActionButtonProps {
  rec: Recommendation;
  actionInProgress: string | null;
  onAction: (rec: Recommendation) => void;
}

export function ActionButton({ rec, actionInProgress, onAction }: ActionButtonProps) {
  if (!rec.action_type) return null;

  const isActive = actionInProgress === rec.action_type;
  const labels: Record<string, { idle: string; active: string }> = {
    pull_embedding_model: { idle: 'Pull Model', active: 'Pulling...' },
    open_settings_interests: { idle: 'Open Settings', active: 'Open Settings' },
    open_settings_stacks: { idle: 'Open Settings', active: 'Open Settings' },
    auto_detect_stacks: { idle: 'Auto-Detect', active: 'Detecting...' },
    install_ollama: { idle: 'Install Ollama', active: 'Installing...' },
    give_feedback: { idle: 'Go to Results', active: 'Go to Results' },
  };
  const label = labels[rec.action_type] || { idle: 'Fix', active: 'Fixing...' };

  return (
    <button
      onClick={() => onAction(rec)}
      disabled={!!actionInProgress && !['open_settings_interests', 'open_settings_stacks', 'give_feedback', 'install_ollama'].includes(rec.action_type!)}
      style={{
        marginTop: 6,
        padding: '4px 12px',
        background: isActive ? '#2A2A2A' : '#D4AF37',
        color: isActive ? '#666666' : '#0A0A0A',
        border: 'none',
        borderRadius: 4,
        fontSize: 11,
        fontWeight: 600,
        cursor: isActive ? 'not-allowed' : 'pointer',
        fontFamily: 'Inter, sans-serif',
      }}
    >
      {isActive ? label.active : label.idle}
    </button>
  );
}

interface RecommendationItemProps {
  rec: Recommendation;
  index: number;
  actionInProgress: string | null;
  onAction: (rec: Recommendation) => void;
}

export function RecommendationItem({ rec, index, actionInProgress, onAction }: RecommendationItemProps) {
  return (
    <div style={{ padding: '8px 0', borderTop: index > 0 ? '1px solid #1F1F1F' : 'none' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{
          fontSize: 10,
          fontWeight: 700,
          color: priorityColor(rec.priority),
          fontFamily: 'JetBrains Mono, monospace',
        }}>
          {rec.priority}
        </span>
        <span style={{ fontSize: 13, fontWeight: 500, color: '#FFFFFF' }}>
          {rec.title}
        </span>
      </div>
      <p style={{ margin: '4px 0 0', fontSize: 12, color: '#A0A0A0', lineHeight: 1.4 }}>
        {rec.description}
      </p>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 4 }}>
        <ActionButton rec={rec} actionInProgress={actionInProgress} onAction={onAction} />
        {rec.action && !rec.action_type && (
          <code style={{
            display: 'inline-block',
            padding: '2px 6px',
            background: '#1F1F1F',
            borderRadius: 3,
            fontSize: 11,
            color: '#D4AF37',
            fontFamily: 'JetBrains Mono, monospace',
          }}>
            {rec.action}
          </code>
        )}
      </div>
    </div>
  );
}
