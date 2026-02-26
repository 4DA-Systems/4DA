import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

export function LicenseSection({ onStatus }: { onStatus: (s: string) => void }) {
  const { t } = useTranslation();
  const tier = useAppStore(s => s.tier);
  const trialStatus = useAppStore(s => s.trialStatus);
  const licenseLoading = useAppStore(s => s.licenseLoading);
  const activateLicense = useAppStore(s => s.activateLicense);
  const startTrial = useAppStore(s => s.startTrial);
  const loadLicense = useAppStore(s => s.loadLicense);
  const loadTrialStatus = useAppStore(s => s.loadTrialStatus);
  const expired = useAppStore(s => s.expired);
  const daysRemaining = useAppStore(s => s.daysRemaining);
  const expiresAt = useAppStore(s => s.expiresAt);

  const [key, setKey] = useState('');
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    loadLicense();
    loadTrialStatus();
  }, [loadLicense, loadTrialStatus]);

  const isPro = !expired && (tier === 'pro' || tier === 'team');
  const trialActive = trialStatus?.active === true;
  const trialExpired = trialStatus != null && !trialStatus.active && trialStatus.started_at != null;
  const canStartTrial = !isPro && !trialStatus?.started_at;
  const expiryWarning = isPro && daysRemaining > 0 && daysRemaining <= 14;

  const handleActivate = async () => {
    if (!key.trim()) return;
    const ok = await activateLicense(key.trim());
    if (ok) {
      onStatus(t('settings.license.activated'));
      setKey('');
    } else {
      onStatus(t('settings.license.invalidKey'));
    }
    setTimeout(() => onStatus(''), 3000);
  };

  const handleStartTrial = async () => {
    setStarting(true);
    const ok = await startTrial();
    setStarting(false);
    if (ok) {
      onStatus(t('settings.license.trialStarted'));
    } else {
      onStatus(t('settings.license.trialError'));
    }
    setTimeout(() => onStatus(''), 3000);
  };

  const tierConfig: Record<string, { label: string; color: string }> = {
    free: { label: t('tier.free'), color: 'text-gray-400' },
    pro: { label: t('tier.pro'), color: 'text-[#D4AF37]' },
    team: { label: t('settings.license.tierTeam'), color: 'text-[#22C55E]' },
  };
  const { label: tierLabel, color: tierColor } = tierConfig[tier] ?? tierConfig.free;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-3">{t('settings.license.title')}</h3>

      {/* Current tier */}
      <div className="flex items-center gap-2 mb-3">
        <span className="text-xs text-gray-500">{t('settings.license.currentTier')}</span>
        <span className={`text-xs font-semibold ${tierColor}`}>{tierLabel}</span>
        {trialActive && (
          <span className="text-[10px] px-1.5 py-0.5 bg-[#D4AF37]/15 text-[#D4AF37] rounded">
            {t('settings.license.trialDaysLeft', { days: trialStatus.days_remaining })}
          </span>
        )}
        {trialExpired && (
          <span className="text-[10px] px-1.5 py-0.5 bg-[#EF4444]/15 text-[#EF4444] rounded">
            {t('settings.license.trialExpired')}
          </span>
        )}
      </div>

      {/* Expired license banner */}
      {expired && (
        <div className="mb-3 p-2.5 rounded-lg bg-[#EF4444]/10 border border-[#EF4444]/30">
          <p className="text-xs font-medium text-[#EF4444] mb-1">{t('settings.license.expired')}</p>
          <p className="text-[10px] text-[#EF4444]/70">
            {expiresAt
              ? t('settings.license.expiredOn', { date: new Date(expiresAt).toLocaleDateString() })
              : t('settings.license.expiredGeneric')}
          </p>
          <a
            href="https://4da.ai/streets"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block mt-2 px-3 py-1.5 text-[10px] font-semibold text-black bg-[#D4AF37] rounded hover:bg-[#C4A030] transition-colors"
          >
            {t('settings.license.renew')}
          </a>
        </div>
      )}

      {/* Expiry warning (< 14 days) */}
      {expiryWarning && (
        <div className="mb-3 p-2.5 rounded-lg bg-[#D4AF37]/10 border border-[#D4AF37]/30">
          <p className="text-[10px] text-[#D4AF37]">
            {t('settings.license.expiresIn', { count: daysRemaining })}{' '}
            <a href="https://4da.ai/streets" target="_blank" rel="noopener noreferrer" className="underline font-medium">
              {t('settings.license.renewNow')}
            </a>
          </p>
        </div>
      )}

      {/* Pro badge — show what's unlocked */}
      {isPro && (
        <p className="text-xs text-gray-500 mb-3">
          {t('settings.license.proUnlocked')}{expiresAt && !expiryWarning ? ` ${t('settings.license.renewsOn', { date: new Date(expiresAt).toLocaleDateString() })}` : ` ${t('settings.license.verified')}`}
        </p>
      )}

      {/* License key input — show when not Pro or expired */}
      {(!isPro || expired) && (
        <div className="space-y-3">
          <div className="flex gap-2">
            <input
              type="text"
              value={key}
              onChange={e => setKey(e.target.value)}
              placeholder="4DA-xxxxx.xxxxx"
              onKeyDown={e => e.key === 'Enter' && handleActivate()}
              className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder-gray-600 focus:outline-none focus:border-[#D4AF37]/50 font-mono text-xs"
            />
            <button
              onClick={handleActivate}
              disabled={licenseLoading || !key.trim()}
              className="px-4 py-2 text-sm font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50"
            >
              {licenseLoading ? '...' : t('action.activate')}
            </button>
          </div>

          {/* Trial button */}
          {canStartTrial && (
            <button
              onClick={handleStartTrial}
              disabled={starting}
              className="w-full px-4 py-2 text-xs font-medium text-gray-300 border border-gray-600 rounded-lg hover:border-gray-400 hover:text-white transition-colors disabled:opacity-50"
            >
              {starting ? t('settings.license.starting') : t('settings.license.startTrial')}
            </button>
          )}

          {/* Upgrade link */}
          <a
            href="https://4da.ai/streets"
            target="_blank"
            rel="noopener noreferrer"
            className="block text-center text-xs text-[#D4AF37] hover:underline"
          >
            {t('settings.license.getKey')}
          </a>
        </div>
      )}
    </div>
  );
}
