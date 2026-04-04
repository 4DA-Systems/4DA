// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLicense } from '../hooks/use-license';
import { useAppStore } from '../store';

interface SignalUpgradeCTAProps {
  compact?: boolean;
}

/**
 * Shared inline upgrade CTA for Signal-gated components.
 * Shows "Upgrade to Signal" button + optional trial start.
 */
export function SignalUpgradeCTA({ compact }: SignalUpgradeCTAProps) {
  const { t } = useTranslation();
  const { trialStatus } = useLicense();
  const startTrial = useAppStore(s => s.startTrial);
  const [starting, setStarting] = useState(false);

  const canStartTrial = !trialStatus?.started_at;

  const handleStartTrial = async () => {
    setStarting(true);
    await startTrial();
    setStarting(false);
  };

  return (
    <div className={`flex items-center justify-center ${compact ? 'gap-2' : 'gap-3'}`}>
      <a
        href="https://4da.ai/signal"
        target="_blank"
        rel="noopener noreferrer"
        className={`font-medium text-black bg-accent-gold rounded-lg hover:bg-[#C4A030] transition-colors ${
          compact ? 'px-3 py-1.5 text-xs' : 'px-4 py-2 text-sm'
        }`}
      >
        {t('pro.upgrade')}
      </a>
      {canStartTrial && (
        <button
          onClick={handleStartTrial}
          disabled={starting}
          className={`font-medium text-text-secondary border border-border rounded-lg hover:border-gray-400 hover:text-white transition-colors disabled:opacity-50 ${
            compact ? 'px-3 py-1.5 text-xs' : 'px-4 py-2 text-sm'
          }`}
        >
          {starting ? t('pro.startingTrial') : t('pro.startTrial')}
        </button>
      )}
    </div>
  );
}
