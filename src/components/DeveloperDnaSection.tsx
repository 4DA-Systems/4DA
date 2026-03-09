import { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../store';
import type { DeveloperDna } from '../types';
import { getSourceFullName } from '../config/sources';

// ============================================================================
// Developer DNA Section (loaded on expand)
// ============================================================================

export function DeveloperDnaSection() {
  const { t } = useTranslation();
  const addToast = useAppStore(s => s.addToast);
  const [dna, setDna] = useState<DeveloperDna | null>(null);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const loaded = useRef(false);

  const loadDna = async () => {
    setLoading(true);
    try {
      const d = await invoke<DeveloperDna>('get_developer_dna');
      setDna(d);
      loaded.current = true;
    } catch {
      // DNA may not be available yet
    } finally {
      setLoading(false);
    }
  };

  const copyDna = async () => {
    try {
      const md = await invoke<string>('export_developer_dna_markdown');
      await window.navigator.clipboard.writeText(md);
      addToast('success', t('profile.dnaCopied'));
    } catch { /* clipboard may fail */ }
  };

  return (
    <div className="border-t border-border pt-4">
      <button
        onClick={() => {
          const willExpand = !expanded;
          setExpanded(willExpand);
          if (willExpand && !loaded.current) loadDna();
        }}
        aria-expanded={expanded}
        aria-label={t('profile.toggleDna')}
        className="flex items-center gap-2 w-full text-left group"
      >
        <span className={`text-text-muted text-xs transition-transform ${expanded ? 'rotate-90' : ''}`}>&#9654;</span>
        <h3 className="text-xs font-medium text-text-muted uppercase tracking-wider group-hover:text-text-secondary transition-colors">
          {t('profile.developerDna')}
        </h3>
      </button>

      {expanded && loading && (
        <div className="flex items-center gap-2 py-6 justify-center">
          <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
          <span className="text-xs text-text-muted">{t('profile.buildingDna')}</span>
        </div>
      )}

      {expanded && dna && !loading && (
        <div className="mt-3 space-y-4">
          {/* Identity */}
          <div className="flex items-center justify-between">
            <p className="text-xs text-text-secondary">{dna.identity_summary}</p>
            <button onClick={copyDna} aria-label={t('profile.copyDna')} className="px-2 py-1 text-[10px] text-white bg-white/10 hover:bg-white/15 border border-white/20 rounded transition-colors">
              {t('profile.copyDna')}
            </button>
          </div>

          {/* Attention Distribution */}
          {dna.top_engaged_topics.length > 0 && (
            <div>
              <h4 className="text-[11px] font-medium text-text-muted mb-2">{t('profile.attentionDistribution')}</h4>
              <div className="space-y-1.5">
                {dna.top_engaged_topics.slice(0, 6).map((topic) => (
                  <div key={topic.topic} className="flex items-center gap-3">
                    <span className="text-[11px] text-text-secondary w-24 truncate">{topic.topic}</span>
                    <div className="flex-1 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                      <div className="h-full bg-white/20 rounded-full" style={{ width: `${Math.min(100, topic.percent_of_total)}%` }} />
                    </div>
                    <span className="text-[10px] text-text-muted w-8 text-right">{topic.percent_of_total.toFixed(0)}%</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Blind Spots */}
          {dna.blind_spots.length > 0 && (
            <div>
              <h4 className="text-[11px] font-medium text-amber-400 mb-1.5">{t('profile.blindSpots')}</h4>
              <div className="flex flex-wrap gap-1.5">
                {dna.blind_spots.slice(0, 5).map((spot) => (
                  <span key={spot.dependency} className="px-2 py-0.5 text-[10px] bg-amber-500/10 text-amber-300 rounded-full border border-amber-500/20">
                    {spot.dependency} ({spot.days_stale}d)
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Source Engagement */}
          {dna.source_engagement.length > 0 && (
            <div>
              <h4 className="text-[11px] font-medium text-text-muted mb-1.5">{t('profile.sourceEngagement')}</h4>
              <div className="grid grid-cols-2 lg:grid-cols-3 gap-2">
                {dna.source_engagement.map((src) => (
                  <div key={src.source_type} className="px-2.5 py-1.5 bg-[#1A1A1A] rounded border border-border">
                    <div className="text-[11px] font-medium text-text-secondary">{getSourceFullName(src.source_type)}</div>
                    <div className="text-[10px] text-text-muted">{t('sovereignProfile.sourceStats', { seen: src.items_seen.toLocaleString(), saved: src.items_saved })}</div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Stats */}
          <div className="flex gap-6 pt-2 border-t border-border/50">
            <div><span className="text-xs text-white">{dna.stats.project_count}</span><span className="text-[10px] text-text-muted ml-1">{t('profile.projects')}</span></div>
            <div><span className="text-xs text-white">{dna.stats.dependency_count}</span><span className="text-[10px] text-text-muted ml-1">{t('profile.deps')}</span></div>
            <div><span className="text-xs text-white">{dna.stats.rejection_rate.toFixed(1)}%</span><span className="text-[10px] text-text-muted ml-1">{t('profile.filtered')}</span></div>
          </div>
        </div>
      )}
    </div>
  );
}
