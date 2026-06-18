// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

/**
 * Toggle for auto-assessing Blind Spots with AI. When on, the Blind Spots lens
 * runs the AI triage whenever the surfaced dependency set changes — cached, so
 * it only spends a model call on an actual change. Optimistic toggle; reverts
 * if the persist fails.
 */
export const BlindSpotsAssessSection = memo(function BlindSpotsAssessSection({
  initialEnabled,
}: {
  initialEnabled: boolean;
}) {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState(initialEnabled);
  const [saving, setSaving] = useState(false);

  const toggle = useCallback(() => {
    const next = !enabled;
    setEnabled(next); // optimistic
    setSaving(true);
    void cmd('set_auto_assess_blind_spots', { enabled: next })
      .catch(() => setEnabled(!next)) // revert on failure
      .finally(() => setSaving(false));
  }, [enabled]);

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <label className="flex items-start gap-3 cursor-pointer group">
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          aria-label={t('settings.ai.autoAssessTitle')}
          disabled={saving}
          onClick={toggle}
          className={`relative w-9 h-5 rounded-full transition-colors shrink-0 mt-0.5 ${enabled ? 'bg-success' : 'bg-border'} disabled:opacity-60`}
        >
          <span
            className={`absolute top-0.5 start-0.5 w-4 h-4 rounded-full bg-white transition-transform ${enabled ? 'translate-x-4' : 'translate-x-0'}`}
          />
        </button>
        <span className="min-w-0">
          <span className="block text-sm font-medium text-text-primary">
            {t('settings.ai.autoAssessTitle')}
          </span>
          <span className="block text-xs text-text-muted mt-0.5 leading-relaxed">
            {t('settings.ai.autoAssessDesc')}
          </span>
        </span>
      </label>
    </div>
  );
});
