// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface TierResponse {
  tier: 'full' | 'good' | 'basic';
  supports_reranking: boolean;
  supports_adversarial: boolean;
  supports_llm_explanations: boolean;
  supports_briefing_synthesis: boolean;
  provider?: string;
  model?: string;
  probed?: boolean;
}

const TIER_CONFIG = {
  full: { label: 'Full', color: 'text-green-400', bg: 'bg-green-500/15', border: 'border-green-500/30' },
  good: { label: 'Good', color: 'text-amber-400', bg: 'bg-amber-500/15', border: 'border-amber-500/30' },
  basic: { label: 'Basic', color: 'text-text-secondary', bg: 'bg-white/5', border: 'border-border' },
} as const;

const FEATURES = [
  { key: 'supports_reranking', label: 'LLM Reranking' },
  { key: 'supports_adversarial', label: 'Adversarial Deliberation' },
  { key: 'supports_llm_explanations', label: 'Analysis Text' },
  { key: 'supports_briefing_synthesis', label: 'Briefing Synthesis' },
] as const;

const TIER_NOTES: Record<TierResponse['tier'], string> = {
  full: 'All intelligence features active — LLM reranking, adversarial deliberation, analysis text, briefing synthesis.',
  good: 'Most intelligence features active. Analysis text quality may vary with this model size.',
  basic: 'Small models use pipeline scoring with heuristic explanations. Upgrade to a larger model for LLM-powered analysis text and adversarial deliberation.',
};

export const ModelTierIndicator = memo(function ModelTierIndicator() {
  const { t } = useTranslation();
  const [data, setData] = useState<TierResponse | null>(null);
  const [probing, setProbing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchTier = useCallback(async () => {
    try {
      setError(null);
      const result = await cmd('get_llm_capability_tier') as TierResponse;
      setData(result);
    } catch {
      setError(t('settings.ai.tierFetchFailed', 'Could not detect model tier'));
    }
  }, [t]);

  useEffect(() => { fetchTier(); }, [fetchTier]);

  const handleProbe = useCallback(async () => {
    setProbing(true);
    try {
      const result = await cmd('probe_llm_capability') as TierResponse;
      setData(result);
    } catch {
      setError(t('settings.ai.probeFailed', 'Capability probe failed'));
    } finally {
      setProbing(false);
    }
  }, [t]);

  if (error && !data) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <p className="text-sm text-text-muted">{error}</p>
      </div>
    );
  }

  if (!data) return null;

  const cfg = TIER_CONFIG[data.tier];

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className={`text-xs font-semibold px-2.5 py-1 rounded-md ${cfg.bg} ${cfg.color} ${cfg.border} border`}>
            {t(`settings.ai.tier.${data.tier}`, cfg.label)}
          </span>
          <span className="text-sm text-text-secondary truncate max-w-[220px]" title={data.model}>
            {data.model || t('settings.ai.unknownModel', 'Unknown model')}
          </span>
        </div>
        <button
          onClick={handleProbe}
          disabled={probing}
          className="text-xs text-text-muted hover:text-white px-2.5 py-1 rounded border border-border hover:border-orange-500/30 transition-all disabled:opacity-50 flex items-center gap-1.5"
        >
          {probing && <span className="w-3 h-3 border-[1.5px] border-current/30 border-t-current rounded-full animate-spin" />}
          {probing ? t('settings.ai.probing', 'Probing...') : t('settings.ai.reprobe', 'Re-probe')}
        </button>
      </div>

      <div className="flex flex-wrap gap-2">
        {FEATURES.map(({ key, label }) => {
          const active = data[key];
          return (
            <span
              key={key}
              className={`text-xs px-2 py-0.5 rounded ${active ? 'bg-green-500/10 text-green-400' : 'bg-white/5 text-text-muted line-through'}`}
            >
              {t(`settings.ai.feature.${key}`, label)}
            </span>
          );
        })}
      </div>

      <p className="text-xs text-text-muted leading-relaxed">
        {t(`settings.ai.tierNote.${data.tier}`, TIER_NOTES[data.tier])}
      </p>

      {data.probed && (
        <p className="text-[10px] text-text-muted opacity-60">
          {t('settings.ai.probed', 'Tier confirmed via live probe')}
        </p>
      )}
    </div>
  );
});
