import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

const FEATURE_KEYS = [
  'coach.gate.featureCoaching',
  'coach.gate.featureEngine',
  'coach.gate.featureStrategy',
  'coach.gate.featureLaunch',
  'coach.gate.featureProgress',
] as const;

export function StreetsGate({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  const streetsTier = useAppStore((s) => s.streetsTier);
  const activateLicense = useAppStore((s) => s.activateStreetsLicense);

  const [licenseKey, setLicenseKey] = useState('');
  const [activating, setActivating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const hasAccess = streetsTier === 'community' || streetsTier === 'cohort';

  const handleActivate = useCallback(async () => {
    if (!licenseKey.trim()) return;
    setActivating(true);
    setError(null);
    const success = await activateLicense(licenseKey.trim());
    setActivating(false);
    if (!success) {
      setError('Invalid license key. Please check and try again.');
    }
  }, [licenseKey, activateLicense]);

  if (hasAccess) {
    return <>{children}</>;
  }

  return (
    <div className="relative">
      <div className="opacity-20 pointer-events-none select-none blur-[3px]" aria-hidden="true">
        {children}
      </div>
      <div className="absolute inset-0 flex items-center justify-center bg-black/80 backdrop-blur-sm">
        <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl px-8 py-7 text-center max-w-sm shadow-2xl">
          <h3 className="text-base font-semibold text-white mb-1">
            {t('coach.gate.title')}
          </h3>
          <p className="text-xs text-[#A0A0A0] mb-4">
            {t('coach.gate.subtitle')}
          </p>

          <div className="inline-block px-2.5 py-1 bg-[#1F1F1F] border border-[#2A2A2A] rounded-md text-[10px] text-[#A0A0A0] mb-5">
            {t('coach.gate.currentTier')} <span className="text-white font-medium capitalize">{streetsTier}</span>
          </div>

          <div className="text-left mb-5">
            <p className="text-[10px] text-[#666] uppercase tracking-wide mb-2 font-medium">
              {t('coach.gate.features')}
            </p>
            <ul className="space-y-1.5">
              {FEATURE_KEYS.map((key) => (
                <li key={key} className="flex items-center gap-2 text-xs text-[#A0A0A0]">
                  <span className="text-[#22C55E] flex-shrink-0">+</span>
                  <span>{t(key)}</span>
                </li>
              ))}
            </ul>
          </div>

          {/* License Key Input */}
          <div className="space-y-2 mb-3">
            <div className="flex gap-2">
              <input
                type="text"
                value={licenseKey}
                onChange={(e) => setLicenseKey(e.target.value)}
                placeholder={t('coach.gate.enterLicenseKey')}
                className="flex-1 bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg px-3 py-2 text-xs text-[#A0A0A0] placeholder-[#666] focus:border-[#D4AF37] focus:outline-none"
                onKeyDown={(e) => e.key === 'Enter' && handleActivate()}
              />
              <button
                onClick={handleActivate}
                disabled={activating || !licenseKey.trim()}
                className="px-3 py-2 text-xs font-medium bg-[#1F1F1F] text-[#A0A0A0] border border-[#2A2A2A] rounded-lg hover:bg-[#2A2A2A] hover:text-white transition-colors disabled:opacity-50"
              >
                {activating ? '...' : t('action.activate')}
              </button>
            </div>
            {error && (
              <p className="text-[10px] text-[#EF4444]">{error}</p>
            )}
          </div>

          {/* Upgrade Link */}
          <a
            href="https://4da.ai/streets"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block w-full px-5 py-2.5 text-sm font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors"
          >
            {t('coach.gate.getCommunity')}
          </a>
        </div>
      </div>
    </div>
  );
}
