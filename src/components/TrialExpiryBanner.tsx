// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useLicense } from '../hooks/use-license';
import { SignalUpgradeCTA } from './SignalUpgradeCTA';

/**
 * Announces the end of the 14-day reverse trial instead of letting it expire
 * silently. Shown in the final stretch (4 days or fewer remaining) to users
 * with no license. Honest copy, no urgency-pressure: states exactly what
 * stays free (the Preemption OSV security floor) and what continues with
 * Signal.
 *
 * Dismissal is persisted per remaining-day count, so the banner reappears
 * once for each of day 4, 3, 2 and 1 as the trial winds down — informative
 * without nagging within a day.
 */
const DISMISS_KEY_PREFIX = '4da-trial-expiry-dismissed-d';
/** First day (days remaining) at which the banner appears. */
const SHOW_AT_DAYS = 4;

export function TrialExpiryBanner() {
  const { t } = useTranslation();
  const { trialStatus } = useLicense();
  const days = trialStatus?.days_remaining ?? 0;
  const dismissKey = `${DISMISS_KEY_PREFIX}${days}`;

  // Dismissal is read against the CURRENT day key on every render —
  // trialStatus loads async, so the key is only meaningful once it arrives,
  // and the banner must reappear when the remaining-day count drops.
  const [, bumpRender] = useState(0);
  let dismissed = false;
  try {
    dismissed = localStorage.getItem(dismissKey) !== null;
  } catch {
    // Storage unavailable — treat as not dismissed.
  }

  const eligible =
    trialStatus !== null &&
    !trialStatus.has_license &&
    trialStatus.active &&
    days >= 1 &&
    days <= SHOW_AT_DAYS;

  if (!eligible || dismissed) return null;

  const dismiss = () => {
    try {
      localStorage.setItem(dismissKey, '1');
    } catch {
      // Storage unavailable — dismissal lasts only this render cycle.
    }
    bumpRender((n) => n + 1);
  };

  return (
    <div className="mx-4 mt-2 mb-1 bg-accent-gold/8 border border-accent-gold/25 rounded-lg overflow-hidden">
      <div className="px-3 py-2 flex items-center justify-between gap-3 flex-wrap">
        <div className="min-w-0">
          <span className="text-sm text-text-primary">
            {t('trialExpiry.title', { count: days })}
          </span>
          <p className="text-xs text-text-muted">{t('trialExpiry.body')}</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <SignalUpgradeCTA compact />
          <button
            onClick={dismiss}
            className="px-3 py-1.5 text-xs rounded bg-text-primary/5 text-text-secondary hover:text-text-primary hover:bg-text-primary/10 transition-colors whitespace-nowrap"
          >
            {t('trialExpiry.dismiss')}
          </button>
        </div>
      </div>
    </div>
  );
}
