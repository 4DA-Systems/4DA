// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { cmd } from '../../lib/commands';

/**
 * The day the running Signal trial ends, formatted for the user's locale, or
 * null when no trial is active. First launch starts the trial automatically,
 * so onboarding states it as running rather than offering to start it.
 */
export function useTrialEndDate(locale?: string): string | null {
  const [endDate, setEndDate] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const status = await cmd('get_trial_status');
        if (cancelled || !status?.active) return;
        const end = new Date(Date.now() + Math.max(0, status.days_remaining) * 86_400_000);
        setEndDate(end.toLocaleDateString(locale, { year: 'numeric', month: 'long', day: 'numeric' }));
      } catch { /* no trial line rather than a wrong one */ }
    })();
    return () => { cancelled = true; };
  }, [locale]);

  return endDate;
}
