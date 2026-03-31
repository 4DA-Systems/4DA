import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { cmd } from '../../lib/commands';
import { formatLocalDate } from '../../utils/format-date';

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

  const recoverLicenseByEmail = useAppStore(s => s.recoverLicenseByEmail);

  const [key, setKey] = useState('');
  const [starting, setStarting] = useState(false);
  const [activationResult, setActivationResult] = useState<{ ok: boolean; reason?: string } | null>(null);
  const [recoveryEmail, setRecoveryEmail] = useState('');
  const [recoveryResult, setRecoveryResult] = useState<{ ok: boolean; reason?: string } | null>(null);
  const [lastValidated, setLastValidated] = useState<string | null>(null);

  useEffect(() => {
    loadLicense();
    loadTrialStatus();
    cmd('get_license_tier').then((result) => {
      const validated = (result as Record<string, unknown>).last_validated_at;
      if (typeof validated === 'string') setLastValidated(validated);
    }).catch((e) => console.debug('[LicenseSection] tier fetch:', e));
  }, [loadLicense, loadTrialStatus]);

  const isPro = !expired && (tier === 'signal' || tier === 'team' || tier === 'enterprise' || tier === 'pro');
  const trialActive = trialStatus?.active === true;
  const trialExpired = trialStatus != null && !trialStatus.active && trialStatus.started_at != null;
  const canStartTrial = !isPro && !trialStatus?.started_at;
  const expiryWarning = isPro && daysRemaining > 0 && daysRemaining <= 14;

  const handleActivate = async () => {
    if (!key.trim()) return;
    setActivationResult(null);
    const result = await activateLicense(key.trim());
    setActivationResult(result);
    if (result.ok) {
      onStatus(t('settings.license.activated'));
      setKey('');
    } else {
      onStatus(result.reason ? t('error.license.validationFailed', { detail: result.reason }) : t('settings.license.invalidKey'));
    }
    setTimeout(() => onStatus(''), 5000);
  };

  const handleRecover = async () => {
    if (!recoveryEmail.trim()) return;
    setRecoveryResult(null);
    const result = await recoverLicenseByEmail(recoveryEmail.trim());
    setRecoveryResult(result);
    if (result.ok) {
      onStatus(t('settings.license.activated'));
      setRecoveryEmail('');
    }
    setTimeout(() => onStatus(''), 5000);
  };

  const recoveryReasonKey: Record<string, string> = {
    not_found: 'settings.license.recovery.notFound',
    expired: 'settings.license.recovery.expired',
    network_error: 'settings.license.recovery.networkError',
    rate_limited: 'settings.license.recovery.rateLimited',
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
    free: { label: t('tier.free'), color: 'text-text-secondary' },
    signal: { label: t('tier.signal'), color: 'text-accent-gold' },
    pro: { label: t('tier.signal'), color: 'text-accent-gold' }, // legacy compat
    team: { label: t('settings.license.tierTeam'), color: 'text-success' },
    enterprise: { label: t('tier.enterprise'), color: 'text-success' },
  };
  const { label: tierLabel, color: tierColor } = (tierConfig[tier] ?? tierConfig.free)!;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-3">{t('settings.license.title')}</h3>

      {/* Current tier */}
      <div className="flex items-center gap-2 mb-3">
        <span className="text-xs text-text-muted">{t('settings.license.currentTier')}</span>
        <span className={`text-xs font-semibold ${tierColor}`}>{tierLabel}</span>
        {trialActive && (
          <span className="text-[10px] px-1.5 py-0.5 bg-accent-gold/15 text-accent-gold rounded">
            {t('settings.license.trialDaysLeft', { days: trialStatus.days_remaining })}
          </span>
        )}
        {trialExpired && (
          <span className="text-[10px] px-1.5 py-0.5 bg-error/15 text-error rounded">
            {t('settings.license.trialExpired')}
          </span>
        )}
      </div>

      {/* Expired license banner */}
      {expired && (
        <div className="mb-3 p-2.5 rounded-lg bg-error/10 border border-error/30">
          <p className="text-xs font-medium text-error mb-1">{t('settings.license.expired')}</p>
          <p className="text-[10px] text-error/70">
            {expiresAt
              ? t('settings.license.expiredOn', { date: formatLocalDate(new Date(expiresAt)) })
              : t('settings.license.expiredGeneric')}
          </p>
          <a
            href="https://4da.ai/signal"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block mt-2 px-3 py-1.5 text-[10px] font-semibold text-black bg-accent-gold rounded hover:bg-[#C4A030] transition-colors"
          >
            {t('settings.license.renew')}
          </a>
        </div>
      )}

      {/* Expiry warning (< 14 days) */}
      {expiryWarning && (
        <div className="mb-3 p-2.5 rounded-lg bg-accent-gold/10 border border-accent-gold/30">
          <p className="text-[10px] text-accent-gold">
            {t('settings.license.expiresIn', { count: daysRemaining })}{' '}
            <a href="https://4da.ai/signal" target="_blank" rel="noopener noreferrer" className="underline font-medium">
              {t('settings.license.renewNow')}
            </a>
          </p>
        </div>
      )}

      {/* Pro badge — show what's unlocked */}
      {isPro && (
        <div className="mb-3">
          <p className="text-xs text-text-muted">
            {t('settings.license.proUnlocked')}{expiresAt && !expiryWarning ? ` ${t('settings.license.renewsOn', { date: formatLocalDate(new Date(expiresAt)) })}` : ` ${t('settings.license.verified')}`}
          </p>
          {lastValidated && (
            <p className="text-[10px] text-text-muted/50 mt-0.5">
              {t('settings.license.lastVerified', { date: formatLocalDate(new Date(lastValidated)) })}
            </p>
          )}
        </div>
      )}

      {/* License key input — show when not Pro or expired */}
      {(!isPro || expired) && (
        <div className="space-y-3">
          <div className="flex gap-2">
            <input
              type="text"
              value={key}
              onChange={e => setKey(e.target.value)}
              placeholder="XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXXXX-V3"
              onKeyDown={e => e.key === 'Enter' && handleActivate()}
              className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder-gray-600 focus:outline-none focus:border-accent-gold/50 font-mono text-xs"
            />
            <button
              onClick={handleActivate}
              disabled={licenseLoading || !key.trim()}
              className="px-4 py-2 text-sm font-medium text-black bg-accent-gold rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50"
            >
              {licenseLoading ? '...' : t('action.activate')}
            </button>
          </div>

          {/* Activation result feedback */}
          {activationResult && (
            <div className={`text-xs p-2 rounded ${activationResult.ok ? 'bg-green-500/10 text-green-400 border border-green-500/30' : 'bg-red-500/10 text-red-400 border border-red-500/30'}`}>
              {activationResult.ok
                ? t('settings.license.activated')
                : activationResult.reason || t('settings.license.invalidKey')}
            </div>
          )}

          {/* License recovery */}
          <details className="border border-border rounded-lg">
            <summary className="px-3 py-2 text-xs text-text-secondary cursor-pointer hover:text-white select-none">
              {t('settings.license.recovery.title')}
            </summary>
            <div className="px-3 pb-3 space-y-2">
              <p className="text-[10px] leading-relaxed text-text-muted">
                {t('settings.license.recovery.explanation')}
              </p>
              <p className="text-[10px] leading-relaxed text-text-muted">
                {t('settings.license.recovery.emailHint')}
              </p>
              <p className="text-[10px] leading-relaxed text-text-muted/60">
                {t('settings.license.recovery.privacy')}
              </p>
              <div className="flex gap-2">
                <input
                  type="email"
                  value={recoveryEmail}
                  onChange={e => setRecoveryEmail(e.target.value)}
                  placeholder={t('settings.license.recovery.placeholder')}
                  onKeyDown={e => e.key === 'Enter' && handleRecover()}
                  className="flex-1 px-3 py-1.5 bg-bg-primary border border-border rounded-lg text-xs text-white placeholder-gray-600 focus:outline-none focus:border-accent-gold/50"
                />
                <button
                  onClick={handleRecover}
                  disabled={licenseLoading || !recoveryEmail.trim()}
                  className="px-3 py-1.5 text-xs font-medium text-black bg-accent-gold rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50"
                >
                  {licenseLoading ? '...' : t('settings.license.recovery.retrieve')}
                </button>
              </div>
              {recoveryResult && (
                <div className={`text-xs p-2 rounded ${recoveryResult.ok ? 'bg-green-500/10 text-green-400 border border-green-500/30' : 'bg-red-500/10 text-red-400 border border-red-500/30'}`}>
                  {recoveryResult.ok
                    ? t('settings.license.recovery.success')
                    : t(recoveryReasonKey[recoveryResult.reason ?? ''] ?? 'settings.license.recovery.networkError')}
                </div>
              )}
            </div>
          </details>

          {/* Trial button */}
          {canStartTrial && (
            <button
              onClick={handleStartTrial}
              disabled={starting}
              className="w-full px-4 py-2 text-xs font-medium text-text-secondary border border-gray-600 rounded-lg hover:border-gray-400 hover:text-white transition-colors disabled:opacity-50"
            >
              {starting ? t('settings.license.starting') : t('settings.license.startTrial')}
            </button>
          )}

          {/* Upgrade link */}
          <a
            href="https://4da.ai/signal"
            target="_blank"
            rel="noopener noreferrer"
            className="block text-center text-xs text-accent-gold hover:underline"
          >
            {t('settings.license.getKey')}
          </a>
        </div>
      )}
    </div>
  );
}
