// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

/**
 * Shown when the license tier probe FAILED and never succeeded this session
 * (licenseLoaded=false + licenseLoadError set) — i.e. the app couldn't reach the
 * license service at cold boot. This is a *transient* condition, NOT a lost key, so
 * the fix is a one-click re-check (loadLicense), not key/email re-entry (that's the
 * separate red LicenseRecoveryBanner for a genuine downgrade).
 *
 * Deliberately obvious and top-level so a paid user who briefly sees the "?" badge
 * has a clear, self-service path to restore their tier without guessing.
 */
export function LicenseUnverifiedBanner() {
  const { t } = useTranslation();
  const licenseLoaded = useAppStore(s => s.licenseLoaded);
  const licenseLoadError = useAppStore(s => s.licenseLoadError);
  const loadLicense = useAppStore(s => s.loadLicense);

  const [rechecking, setRechecking] = useState(false);

  // Only surface the *unverified* state — a confirmed tier (loaded=true), including a
  // confirmed Free, never shows this. Prevents nagging genuine Free users.
  if (licenseLoaded || licenseLoadError === null) return null;

  const handleRecheck = async () => {
    setRechecking(true);
    await loadLicense();
    setRechecking(false);
  };

  return (
    <div className="mx-4 mt-2 mb-1 bg-amber-500/8 border border-amber-500/30 rounded-lg overflow-hidden">
      <div className="px-3 py-2 flex items-center gap-3 flex-wrap">
        <div className="w-2 h-2 rounded-full bg-amber-400 animate-pulse shrink-0" />
        <div className="flex-1 min-w-[200px]">
          <span className="text-sm font-medium text-amber-300">
            {t('license.unverified.title', "Couldn't verify your license")}
          </span>
          <p className="text-xs text-text-secondary mt-0.5">
            {t('license.unverified.description', "The app couldn't reach the license service at startup. Your key is stored locally and has NOT been lost - this is usually a transient timeout. Re-check to restore your tier.")}
          </p>
        </div>
        <button
          onClick={() => void handleRecheck()}
          disabled={rechecking}
          className="px-3 py-1.5 text-xs font-semibold rounded bg-amber-400 text-bg-primary hover:bg-amber-300 transition-colors disabled:opacity-50 shrink-0"
        >
          {rechecking
            ? t('license.unverified.rechecking', 'Re-checking...')
            : t('license.unverified.recheck', 'Re-check license')}
        </button>
      </div>
    </div>
  );
}
