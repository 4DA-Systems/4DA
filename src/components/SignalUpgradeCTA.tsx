// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLicense } from '../hooks/use-license';
import { useAppStore } from '../store';

interface SignalUpgradeCTAProps {
  compact?: boolean;
  /**
   * Which app surface rendered this CTA. Becomes `utm_content` on the 4da.ai
   * link so the SITE's existing analytics can attribute the visit per surface.
   * The app itself sends nothing and embeds no analytics — the only thing that
   * leaves the machine is a URL the user chose to open in their own browser.
   */
  source?: string;
}

/**
 * Shared inline upgrade CTA for Signal-gated components.
 *
 * "Upgrade to Signal" opens 4da.ai/signal. The trial button appears only when
 * a trial can really start (`trialOffer === 'available'`): the trial starts by
 * itself on first launch, so for nearly every user it has already run. A used
 * trial is stated on the full-size paywalls instead of offered again, and a
 * start that fails says so.
 */
export function SignalUpgradeCTA({ compact, source = 'unknown' }: SignalUpgradeCTAProps) {
  const { t } = useTranslation();
  const { trialOffer } = useLicense();
  const startTrial = useAppStore(s => s.startTrial);
  const [starting, setStarting] = useState(false);
  const [startFailed, setStartFailed] = useState(false);

  const handleStartTrial = async () => {
    setStarting(true);
    setStartFailed(false);
    // A refused start re-reads the trial status (license-slice): when a trial
    // had already run, the offer is withdrawn and the used-trial line shows.
    const started = await startTrial();
    setStarting(false);
    setStartFailed(!started);
  };

  const upgradeUrl = `https://4da.ai/signal?utm_source=app&utm_medium=upsell&utm_content=${encodeURIComponent(source)}`;

  return (
    <div className={`flex flex-col items-center ${compact ? 'gap-1' : 'gap-2'}`}>
      <div className={`flex items-center justify-center ${compact ? 'gap-2' : 'gap-3'}`}>
        <a
          href={upgradeUrl}
          target="_blank"
          rel="noopener noreferrer"
          className={`font-medium text-bg-primary bg-accent-gold rounded-lg hover:bg-accent-gold-hover transition-colors ${
            compact ? 'px-3 py-1.5 text-xs' : 'px-4 py-2 text-sm'
          }`}
        >
          {t('pro.upgrade')}
        </a>
        {trialOffer === 'available' && (
          <button
            onClick={() => { void handleStartTrial(); }}
            disabled={starting}
            className={`font-medium text-text-secondary border border-border rounded-lg hover:border-gray-400 hover:text-text-primary transition-colors disabled:opacity-50 ${
              compact ? 'px-3 py-1.5 text-xs' : 'px-4 py-2 text-sm'
            }`}
          >
            {starting ? t('pro.startingTrial') : t('pro.startTrial')}
          </button>
        )}
      </div>
      {trialOffer === 'used' && (!compact || startFailed) && (
        <p className="text-xs text-text-muted" data-testid="signal-cta-trial-used">{t('pro.trialEnded')}</p>
      )}
      {startFailed && trialOffer !== 'used' && trialOffer !== 'active' && (
        <p role="alert" className="text-xs text-error">{t('pro.trialStartFailed')}</p>
      )}
    </div>
  );
}
