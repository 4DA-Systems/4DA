import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLicense } from '../hooks/use-license';
import { useAppStore } from '../store';

interface ProGateProps {
  children: React.ReactNode;
  feature?: string;
}

export function ProGate({ children, feature }: ProGateProps) {
  const { t } = useTranslation();
  const { isPro, trialStatus, expired, daysRemaining } = useLicense();
  const startTrial = useAppStore((s) => s.startTrial);
  const activateLicense = useAppStore((s) => s.activateLicense);
  const [starting, setStarting] = useState(false);
  const [licenseKey, setLicenseKey] = useState('');
  const [activating, setActivating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showKeyInput, setShowKeyInput] = useState(false);

  // Active Pro with expiry warning — show content with a banner
  if (isPro && daysRemaining > 0 && daysRemaining <= 7) {
    return (
      <div>
        <div className="mb-2 px-3 py-1.5 rounded-lg bg-[#D4AF37]/10 border border-[#D4AF37]/20 text-center">
          <p className="text-[10px] text-[#D4AF37]">
            {t('pro.licenseExpiresSoon', { count: daysRemaining })}{' '}
            <a href="https://4da.ai/signal" target="_blank" rel="noopener noreferrer" className="underline font-medium">{t('pro.renew')}</a>
          </p>
        </div>
        {children}
      </div>
    );
  }

  if (isPro) {
    return <>{children}</>;
  }

  const trialExpired = trialStatus && !trialStatus.active && trialStatus.started_at;
  const licenseExpired = expired;
  const canStartTrial = !trialStatus?.started_at;

  const handleStartTrial = async () => {
    setStarting(true);
    await startTrial();
    setStarting(false);
  };

  const handleActivate = async () => {
    if (!licenseKey.trim()) return;
    setActivating(true);
    setError(null);
    const success = await activateLicense(licenseKey.trim());
    setActivating(false);
    if (!success) {
      setError(t('pro.invalidLicenseKey'));
    }
  };

  return (
    <div className="relative">
      <div className="opacity-30 pointer-events-none select-none blur-[2px]" aria-hidden="true">
        {children}
      </div>
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="bg-bg-secondary/95 backdrop-blur-sm border border-[#D4AF37]/30 rounded-xl px-6 py-5 text-center max-w-sm shadow-lg">
          <div className="flex items-center justify-center gap-2 mb-3">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-[#D4AF37]">
              <path d="M8 1L10 6H15L11 9.5L12.5 15L8 11.5L3.5 15L5 9.5L1 6H6L8 1Z" fill="currentColor"/>
            </svg>
            <span className="text-sm font-semibold text-[#D4AF37] tracking-wide uppercase">{t('tier.signal')}</span>
          </div>
          <p className="text-sm text-text-secondary mb-1">
            {feature ? t('pro.featureGated', { feature }) : t('pro.genericGated')}
          </p>
          <p className="text-xs text-text-muted mb-4">
            {licenseExpired
              ? t('pro.licenseExpired')
              : trialExpired
              ? t('pro.trialEnded')
              : t('pro.upgradeDescription')}
          </p>
          <div className="flex flex-col gap-2">
            <a
              href="https://4da.ai/signal"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-block px-5 py-2 text-sm font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors"
            >
              {t('pro.upgrade')}
            </a>
            {canStartTrial && (
              <button
                onClick={handleStartTrial}
                disabled={starting}
                className="px-5 py-2 text-sm font-medium text-text-secondary border border-gray-600 rounded-lg hover:border-gray-400 hover:text-white transition-colors disabled:opacity-50"
              >
                {starting ? t('pro.startingTrial') : t('pro.startTrial')}
              </button>
            )}

            {/* License key activation */}
            {!showKeyInput ? (
              <button
                onClick={() => setShowKeyInput(true)}
                className="text-xs text-text-muted hover:text-[#D4AF37] transition-colors"
              >
                {t('pro.haveLicenseKey')}
              </button>
            ) : (
              <div className="mt-1 space-y-2">
                <div className="flex gap-1.5">
                  <input
                    type="text"
                    value={licenseKey}
                    onChange={e => setLicenseKey(e.target.value)}
                    placeholder="4DA-xxxxx.xxxxx"
                    onKeyDown={e => e.key === 'Enter' && handleActivate()}
                    className="flex-1 bg-bg-primary border border-border rounded-lg px-2.5 py-1.5 text-xs text-white placeholder-[#8A8A8A] focus:border-[#D4AF37] focus:outline-none font-mono"
                  />
                  <button
                    onClick={handleActivate}
                    disabled={activating || !licenseKey.trim()}
                    className="px-3 py-1.5 text-xs font-medium bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:bg-[#2A2A2A] hover:text-white transition-colors disabled:opacity-50"
                  >
                    {activating ? '...' : t('action.activate')}
                  </button>
                </div>
                {error && <p className="text-[10px] text-[#EF4444]">{error}</p>}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
