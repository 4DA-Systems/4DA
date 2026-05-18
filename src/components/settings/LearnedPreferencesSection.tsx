// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';

interface LearnedFacet {
  facet_id: string;
  class: string;
  key: string;
  value: string;
  stability: number;
  state: string;
  user_state: string;
  evidence_count: number;
  first_seen_at: number;
  last_seen_at: number;
}

const DEFAULT_CLASS = { label: 'Other', color: 'bg-zinc-500/15 text-zinc-400 border-zinc-500/25', activeColor: 'bg-zinc-500/25 border-zinc-500/40' };

const CLASS_CONFIG: Record<string, { label: string; color: string; activeColor: string }> = {
  interest: { label: 'Interests', color: 'bg-indigo-500/15 text-indigo-400 border-indigo-500/25', activeColor: 'bg-indigo-500/25 border-indigo-500/40' },
  source_pref: { label: 'Sources', color: 'bg-cyan-500/15 text-cyan-400 border-cyan-500/25', activeColor: 'bg-cyan-500/25 border-cyan-500/40' },
  topic_affinity: { label: 'Topics', color: 'bg-amber-500/15 text-amber-400 border-amber-500/25', activeColor: 'bg-amber-500/25 border-amber-500/40' },
  veto: { label: 'Filtered', color: 'bg-red-500/15 text-red-400 border-red-500/25', activeColor: 'bg-red-500/25 border-red-500/40' },
  workflow: { label: 'Workflow', color: 'bg-green-500/15 text-green-400 border-green-500/25', activeColor: 'bg-green-500/25 border-green-500/40' },
  temporal: { label: 'Timing', color: 'bg-violet-500/15 text-violet-400 border-violet-500/25', activeColor: 'bg-violet-500/25 border-violet-500/40' },
};

function Chip({
  facet,
  selected,
  onSelect,
}: {
  facet: LearnedFacet;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  const isPinned = facet.user_state === 'pinned';
  const isForgotten = facet.user_state === 'forgotten';
  const cfg = CLASS_CONFIG[facet.class] ?? DEFAULT_CLASS;

  return (
    <button
      onClick={() => onSelect(facet.facet_id)}
      className={`
        inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border text-xs font-medium
        transition-all cursor-pointer select-none
        ${selected ? 'ring-1 ring-white/30 ' + cfg.activeColor : cfg.color}
        ${isForgotten ? 'opacity-40 line-through' : ''}
        hover:brightness-125
      `}
    >
      {isPinned && <span className="text-[10px]">&#x1F4CC;</span>}
      <span className="truncate max-w-[140px]">{facet.value || facet.key}</span>
      {facet.stability >= 5.0 && <span className="w-1.5 h-1.5 rounded-full bg-green-400 flex-shrink-0" title="Strong signal" />}
    </button>
  );
}

function ChipActions({
  facet,
  onAction,
}: {
  facet: LearnedFacet;
  onAction: (action: 'pin_preference' | 'forget_preference' | 'reset_preference') => void;
}) {
  const { t } = useTranslation();
  const isPinned = facet.user_state === 'pinned';
  const isForgotten = facet.user_state === 'forgotten';
  const hasOverride = isPinned || isForgotten;

  return (
    <div className="flex items-center gap-4 px-1 py-2 text-xs animate-in fade-in duration-150">
      <div className="flex-1 min-w-0">
        <span className="text-white font-medium">{facet.value || facet.key}</span>
        <span className="text-text-muted ml-2">
          {facet.evidence_count} signal{facet.evidence_count !== 1 ? 's' : ''}
          {' · '}
          {facet.state}
        </span>
      </div>
      <div className="flex gap-1.5 flex-shrink-0">
        {hasOverride ? (
          <button
            onClick={() => onAction('reset_preference')}
            className="px-2.5 py-1 rounded-md bg-zinc-500/10 text-zinc-400 border border-zinc-500/20 hover:bg-zinc-500/20 transition-colors"
          >
            {t('learnedPreferences.resetButton')}
          </button>
        ) : (
          <>
            <button
              onClick={() => onAction('pin_preference')}
              className="px-2.5 py-1 rounded-md bg-blue-500/10 text-blue-400 border border-blue-500/20 hover:bg-blue-500/20 transition-colors"
              title={t('learnedPreferences.pinTitle')}
            >
              {t('learnedPreferences.pinButton')}
            </button>
            <button
              onClick={() => onAction('forget_preference')}
              className="px-2.5 py-1 rounded-md bg-red-500/10 text-red-400 border border-red-500/20 hover:bg-red-500/20 transition-colors"
              title={t('learnedPreferences.forgetTitle')}
            >
              {t('learnedPreferences.forgetButton')}
            </button>
          </>
        )}
      </div>
    </div>
  );
}

export function LearnedPreferencesSection() {
  const { t } = useTranslation();
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);
  const [facets, setFacets] = useState<LearnedFacet[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const loadPreferences = useCallback(async () => {
    try {
      const result = await cmd('get_learned_preferences');
      setFacets(result.facets);
    } catch {
      setFacets([]);
    }
    setLoading(false);
  }, []);

  useEffect(() => { void loadPreferences(); }, [loadPreferences]);

  const handleAction = async (action: 'pin_preference' | 'forget_preference' | 'reset_preference', facetId: string) => {
    try {
      await cmd(action, { facet_id: facetId });
      await loadPreferences();
      const verb = action === 'pin_preference' ? 'pinned' : action === 'forget_preference' ? 'forgotten' : 'reset';
      setSettingsStatus(`Preference ${verb}`);
      setSelectedId(null);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  const toggleSelect = (id: string) => {
    setSelectedId(prev => prev === id ? null : id);
  };

  const selectedFacet = facets.find(f => f.facet_id === selectedId);

  const grouped = facets.reduce<Record<string, LearnedFacet[]>>((acc, f) => {
    const key = f.user_state === 'forgotten' ? 'veto' : f.class;
    (acc[key] ??= []).push(f);
    return acc;
  }, {});

  const groupOrder = ['interest', 'topic_affinity', 'source_pref', 'workflow', 'temporal', 'veto'];
  const activeGroups = groupOrder.filter(k => grouped[k]?.length);

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-start gap-3 mb-3">
        <div className="w-8 h-8 bg-indigo-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-indigo-400">&#x1F9E0;</span>
        </div>
        <div className="flex-1">
          <h3 className="text-white font-medium">{t('learnedPreferences.title')}</h3>
          <p className="text-text-muted text-sm mt-1">{t('learnedPreferences.subtitle')}</p>
        </div>
        <button
          onClick={() => { setLoading(true); void loadPreferences(); }}
          className="text-xs px-2.5 py-1.5 bg-bg-secondary border border-border rounded-lg text-text-muted hover:text-white hover:border-indigo-500/30 transition-all"
        >
          {t('learnedPreferences.refresh')}
        </button>
      </div>

      {loading ? (
        <div className="text-sm text-text-muted py-4 text-center">{t('learnedPreferences.loading')}</div>
      ) : facets.length === 0 ? (
        <div className="text-sm text-text-muted bg-bg-secondary rounded-lg p-4 text-center border border-border">
          <div className="text-2xl mb-2">&#x1F50D;</div>
          <div>{t('learnedPreferences.empty')}</div>
          <div className="text-xs text-text-muted mt-1">{t('learnedPreferences.emptyHint')}</div>
        </div>
      ) : (
        <div className="space-y-3">
          {activeGroups.map(groupKey => {
            const cfg = CLASS_CONFIG[groupKey] ?? DEFAULT_CLASS;
            const items = grouped[groupKey]!;
            return (
              <div key={groupKey}>
                <div className="text-[11px] text-text-muted font-medium uppercase tracking-wider mb-1.5">
                  {cfg.label}
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {items.map(f => (
                    <Chip
                      key={f.facet_id}
                      facet={f}
                      selected={selectedId === f.facet_id}
                      onSelect={toggleSelect}
                    />
                  ))}
                </div>
              </div>
            );
          })}

          {selectedFacet && (
            <div className="bg-bg-secondary rounded-lg border border-border px-3">
              <ChipActions
                facet={selectedFacet}
                onAction={(action) => { void handleAction(action, selectedFacet.facet_id); }}
              />
            </div>
          )}

          <div className="text-[10px] text-text-muted pt-1">
            {t('learnedPreferences.chipHint')}
          </div>
        </div>
      )}
    </div>
  );
}
