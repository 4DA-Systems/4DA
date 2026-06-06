// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import type { Settings } from '../../types';

/**
 * Morning-brief narration status for the AI settings panel.
 *
 * Whether the brief is AI-narrated or served as 4DA's deterministic grounded floor is
 * decided by the backend gate (`get_brief_capability` → `compute_has_llm` AND
 * `is_brief_capable`) — the SAME gate `digest_commands` runs to pick the brief path. We
 * call that command rather than re-deriving the rule in TS so the hint can never drift
 * from what the next brief actually does.
 *
 * Reflects the SAVED config (re-fetches when the saved provider/model/key changes, i.e.
 * after the user saves a change). The deterministic brief is framed as a real, grounded
 * product — private, offline-capable, never fabricated — not a failure state.
 */
export function BriefNarrationStatus({ settings }: { settings: Settings | null }) {
  const { t } = useTranslation();
  const [cap, setCap] = useState<{
    brief_capable: boolean;
    reason: 'no_llm' | 'model_too_weak' | 'capable';
    provider: string;
    model: string;
  } | null>(null);

  useEffect(() => {
    cmd('get_brief_capability').then(setCap).catch(() => setCap(null));
  }, [settings?.llm.provider, settings?.llm.model, settings?.llm.has_api_key]);

  if (!cap) return null;

  const capable = cap.reason === 'capable';
  const body = capable
    ? t('settings.ai.briefNarrated')
    : cap.reason === 'no_llm'
      ? t('settings.ai.briefFloorNoLlm')
      : t('settings.ai.briefFloorWeak');

  return (
    <div
      className={`p-3 rounded-lg border ${
        capable ? 'bg-green-900/15 border-green-500/30' : 'bg-amber-900/15 border-amber-500/30'
      }`}
    >
      <div className="flex items-center gap-2 mb-1">
        <span
          className={`w-1.5 h-1.5 rounded-full ${capable ? 'bg-green-400' : 'bg-amber-400'}`}
          aria-hidden="true"
        />
        <p className={`text-xs font-medium ${capable ? 'text-green-400' : 'text-amber-400'}`}>
          {t('settings.ai.briefStatusTitle')}
        </p>
      </div>
      <p className="text-xs text-text-muted leading-relaxed">{body}</p>
    </div>
  );
}
