import { memo, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
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
  { label: 'Stack',      key: 'stack',      weight: 0.4, color: '#D4AF37' },
  { label: 'Engagement', key: 'engagement', weight: 0.3, color: '#22C55E' },
  { label: 'Trend',      key: 'trend',      weight: 0.2, color: '#3B82F6' },
  { label: 'Decision',   key: 'decision',   weight: 0.1, color: '#A855F7' },
];

// ============================================================================
// Helpers
// ============================================================================

function movementIcon(movement: RadarEntry['movement']): { icon: string; color: string; label: string } {
  switch (movement) {
    case 'up':     return { icon: '\u25B2', color: '#22C55E', label: 'Moving In' };
    case 'down':   return { icon: '\u25BC', color: '#EF4444', label: 'Moving Out' };
    case 'new':    return { icon: '\u25C6', color: '#D4AF37', label: 'New Entry' };
    case 'stable': return { icon: '\u25CF', color: '#666666', label: 'Stable' };
  }
}

function ringColor(ring: string): string {
  switch (ring) {
    case 'adopt':  return '#22C55E';
    case 'trial':  return '#3B82F6';
    case 'assess': return '#D4AF37';
    case 'hold':   return '#EF4444';
    default:       return '#666666';
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
    invoke<EntryDetail>('get_radar_entry_detail', { name: entry.name })
      .then(setDetail)
      .catch(() => setDetail(null))
      .finally(() => setLoading(false));
  }, [entry]);

  if (!entry) return null;

  const mv = movementIcon(entry.movement);
  const rColor = ringColor(entry.ring);

  return (
    <div
      className="fixed right-0 top-0 h-full w-80 z-50 border-l border-[#2A2A2A] bg-[#141414] shadow-xl overflow-y-auto animate-slide-in"
      style={{ fontFamily: 'Inter, sans-serif' }}
    >
      {/* Header */}
      <div className="flex items-start justify-between p-4 border-b border-[#2A2A2A]">
        <div className="flex-1 min-w-0">
          <h2 className="text-lg font-semibold text-white truncate">{entry.name}</h2>
          <div className="flex items-center gap-2 mt-1.5">
            <span
              className="px-1.5 py-0.5 rounded text-[10px] font-medium"
              style={{ backgroundColor: rColor + '20', color: rColor, border: `1px solid ${rColor}40` }}
            >
              {entry.ring.charAt(0).toUpperCase() + entry.ring.slice(1)}
            </span>
            <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-[#1F1F1F] text-[#A0A0A0] border border-[#2A2A2A]">
              {entry.quadrant.charAt(0).toUpperCase() + entry.quadrant.slice(1)}
            </span>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-1 text-[#666666] hover:text-white transition-colors"
          aria-label="Close panel"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      </div>

      {/* Movement */}
      <div className="px-4 py-3 border-b border-[#2A2A2A] flex items-center gap-2">
        <span style={{ color: mv.color }} className="text-sm">{mv.icon}</span>
        <span className="text-xs text-[#A0A0A0]">{mv.label}</span>
        <span className="ml-auto text-xs font-mono text-[#666666]">
          Score: {entry.score.toFixed(2)}
        </span>
      </div>

      {/* Score Breakdown */}
      <div className="px-4 py-3 border-b border-[#2A2A2A]">
        <h3 className="text-[11px] text-[#666666] uppercase tracking-wide mb-2">{t('techRadar.scoreBreakdown')}</h3>
        <div className="space-y-2">
          {SCORE_WEIGHTS.map(({ label, weight, color }) => (
            <div key={label} className="flex items-center gap-2">
              <span className="text-[10px] text-[#A0A0A0] w-16 flex-shrink-0">{label}</span>
              <div className="flex-1 h-1.5 bg-[#1F1F1F] rounded-full overflow-hidden">
                <div
                  className="h-full rounded-full transition-all duration-500"
                  style={{
                    width: `${weight * 100}%`,
                    backgroundColor: color,
                    opacity: 0.8,
                  }}
                />
              </div>
              <span className="text-[10px] text-[#666666] w-8 text-right font-mono">
                {(weight * 100).toFixed(0)}%
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Signals */}
      {entry.signals.length > 0 && (
        <div className="px-4 py-3 border-b border-[#2A2A2A]">
          <h3 className="text-[11px] text-[#666666] uppercase tracking-wide mb-2">{t('techRadar.signalTrail')}</h3>
          <ul className="space-y-1">
            {entry.signals.map((signal, i) => (
              <li key={i} className="text-xs text-[#A0A0A0] flex items-start gap-1.5">
                <span className="text-[#666666] mt-0.5 flex-shrink-0">&bull;</span>
                <span>{signal}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Decision Reference */}
      {entry.decision_ref !== null && (
        <div className="px-4 py-3 border-b border-[#2A2A2A]">
          <h3 className="text-[11px] text-[#666666] uppercase tracking-wide mb-2">{t('techRadar.decision')}</h3>
          {loading ? (
            <div className="text-xs text-[#666666]">{t('action.loading')}</div>
          ) : detail?.decision ? (
            <div className="space-y-1">
              <div className="text-xs text-[#D4AF37] font-medium">
                Decision #{detail.decision.id}
              </div>
              <div className="text-xs text-[#A0A0A0]">{detail.decision.decision}</div>
              {detail.decision.rationale && (
                <div className="text-[10px] text-[#666666] italic mt-1">{detail.decision.rationale}</div>
              )}
            </div>
          ) : (
            <div className="text-xs text-[#D4AF37]">Decision #{entry.decision_ref}</div>
          )}
        </div>
      )}

      {/* Related Items */}
      {detail && detail.related_items.length > 0 && (
        <div className="px-4 py-3">
          <h3 className="text-[11px] text-[#666666] uppercase tracking-wide mb-2">
            {t('techRadar.recentMentions', { count: detail.related_items.length })}
          </h3>
          <div className="space-y-2">
            {detail.related_items.slice(0, 5).map((item, i) => (
              <div key={i} className="text-xs">
                <div className="text-[#A0A0A0] truncate">{item.title}</div>
                <div className="text-[10px] text-[#666666] flex items-center gap-1">
                  <span>{item.source_type}</span>
                  <span>&middot;</span>
                  <span>{new Date(item.created_at).toLocaleDateString()}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
});
