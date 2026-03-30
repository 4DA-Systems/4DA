import { memo, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { formatLocalDate } from '../../utils/format-date';
import type { RadarEntry } from './RadarSVG';

// ============================================================================
// Types
// ============================================================================

interface EntryDetail {
  entry: RadarEntry | null;
  related_items: Array<{
    title: string;
    source_type: string;
    url: string | null;
    created_at: string;
  }>;
  decision: {
    id: number;
    decision: string;
    rationale: string | null;
    status: string;
  } | null;
}

export interface RadarEntryPanelProps {
  entry: RadarEntry | null;
  onClose: () => void;
}

// ============================================================================
// Score Weights (matches backend EntryBuilder::score())
// ============================================================================

const SCORE_WEIGHTS = [
  { labelKey: 'techRadar.weightStack',      key: 'stack',      weight: 0.4, color: '#D4AF37' },
  { labelKey: 'techRadar.weightEngagement', key: 'engagement', weight: 0.3, color: '#22C55E' },
  { labelKey: 'techRadar.weightTrend',      key: 'trend',      weight: 0.2, color: '#3B82F6' },
  { labelKey: 'techRadar.weightDecision',   key: 'decision',   weight: 0.1, color: '#A855F7' },
];

// ============================================================================
// Helpers
// ============================================================================

function movementIcon(movement: RadarEntry['movement']): { icon: string; color: string; labelKey: string } {
  switch (movement) {
    case 'up':     return { icon: '\u25B2', color: '#22C55E', labelKey: 'techRadar.movementIn' };
    case 'down':   return { icon: '\u25BC', color: '#EF4444', labelKey: 'techRadar.movementOut' };
    case 'new':    return { icon: '\u25C6', color: '#D4AF37', labelKey: 'techRadar.movementNew' };
    case 'stable': return { icon: '\u25CF', color: '#8A8A8A', labelKey: 'techRadar.movementStable' };
  }
}

function ringColor(ring: string): string {
  switch (ring) {
    case 'adopt':  return '#22C55E';
    case 'trial':  return '#3B82F6';
    case 'assess': return '#D4AF37';
    case 'hold':   return '#EF4444';
    default:       return '#8A8A8A';
  }
}

// ============================================================================
// Component
// ============================================================================

export const RadarEntryPanel = memo(function RadarEntryPanel({ entry, onClose }: RadarEntryPanelProps) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<EntryDetail | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!entry) {
      setDetail(null);
      return;
    }
    setLoading(true);
    cmd('get_radar_entry_detail', { name: entry.name })
      .then(r => r as unknown as EntryDetail)
      .then(setDetail)
      .catch(() => setDetail(null))
      .finally(() => setLoading(false));
  }, [entry]);

  if (!entry) return null;

  const mv = movementIcon(entry.movement);
  const rColor = ringColor(entry.ring);

  return (
    <div
      className="fixed end-0 top-0 h-full w-80 z-50 border-s border-border bg-bg-secondary shadow-xl overflow-y-auto animate-slide-in"
      style={{ fontFamily: 'Inter, sans-serif' }}
    >
      {/* Header */}
      <div className="flex items-start justify-between p-4 border-b border-border">
        <div className="flex-1 min-w-0">
          <h2 className="text-lg font-semibold text-white truncate">{entry.name}</h2>
          <div className="flex items-center gap-2 mt-1.5">
            <span
              className="px-1.5 py-0.5 rounded text-[10px] font-medium"
              style={{ backgroundColor: rColor + '20', color: rColor, border: `1px solid ${rColor}40` }}
            >
              {entry.ring.charAt(0).toUpperCase() + entry.ring.slice(1)}
            </span>
            <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-bg-tertiary text-text-secondary border border-border">
              {entry.quadrant.charAt(0).toUpperCase() + entry.quadrant.slice(1)}
            </span>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-1 text-text-muted hover:text-white transition-colors"
          aria-label={t('techRadar.closePanel')}
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      </div>

      {/* Movement */}
      <div className="px-4 py-3 border-b border-border flex items-center gap-2">
        <span style={{ color: mv.color }} className="text-sm">{mv.icon}</span>
        <span className="text-xs text-text-secondary">{t(mv.labelKey)}</span>
        <span className="ms-auto text-xs font-mono text-text-muted">
          {t('techRadar.score', { score: entry.score.toFixed(2) })}
        </span>
      </div>

      {/* Score Breakdown */}
      <div className="px-4 py-3 border-b border-border">
        <h3 className="text-[11px] text-text-muted uppercase tracking-wide mb-2">{t('techRadar.scoreBreakdown')}</h3>
        <div className="space-y-2">
          {SCORE_WEIGHTS.map(({ labelKey, key: wKey, weight, color }) => (
            <div key={wKey} className="flex items-center gap-2">
              <span className="text-[10px] text-text-secondary w-16 flex-shrink-0">{t(labelKey)}</span>
              <div className="flex-1 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                <div
                  className="h-full rounded-full transition-all duration-500"
                  style={{
                    width: `${weight * 100}%`,
                    backgroundColor: color,
                    opacity: 0.8,
                  }}
                />
              </div>
              <span className="text-[10px] text-text-muted w-8 text-end font-mono">
                {(weight * 100).toFixed(0)}%
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Signals */}
      {entry.signals.length > 0 && (
        <div className="px-4 py-3 border-b border-border">
          <h3 className="text-[11px] text-text-muted uppercase tracking-wide mb-2">{t('techRadar.signalTrail')}</h3>
          <ul className="space-y-1">
            {entry.signals.map((signal, i) => (
              <li key={i} className="text-xs text-text-secondary flex items-start gap-1.5">
                <span className="text-text-muted mt-0.5 flex-shrink-0">&bull;</span>
                <span>{signal}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Decision Reference */}
      {entry.decision_ref !== null && (
        <div className="px-4 py-3 border-b border-border">
          <h3 className="text-[11px] text-text-muted uppercase tracking-wide mb-2">{t('techRadar.decision')}</h3>
          {loading ? (
            <div className="text-xs text-text-muted">{t('action.loading')}</div>
          ) : detail?.decision ? (
            <div className="space-y-1">
              <div className="text-xs text-accent-gold font-medium">
                {t('techRadar.decisionRef', { id: detail.decision.id })}
              </div>
              <div className="text-xs text-text-secondary">{detail.decision.decision}</div>
              {detail.decision.rationale && (
                <div className="text-[10px] text-text-muted italic mt-1">{detail.decision.rationale}</div>
              )}
            </div>
          ) : (
            <div className="text-xs text-accent-gold">{t('techRadar.decisionRef', { id: entry.decision_ref })}</div>
          )}
        </div>
      )}

      {/* Related Items */}
      {detail && detail.related_items.length > 0 && (
        <div className="px-4 py-3">
          <h3 className="text-[11px] text-text-muted uppercase tracking-wide mb-2">
            {t('techRadar.recentMentions', { count: detail.related_items.length })}
          </h3>
          <div className="space-y-2">
            {detail.related_items.slice(0, 5).map((item, i) => (
              <div key={i} className="text-xs">
                <div className="text-text-secondary truncate">{item.title}</div>
                <div className="text-[10px] text-text-muted flex items-center gap-1">
                  <span>{item.source_type}</span>
                  <span>&middot;</span>
                  <span>{formatLocalDate(item.created_at)}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
});
