// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

export function LicenseRecoveryBanner() {
  const { t } = useTranslation();
  const wasDowngraded = useAppStore(s => s.wasDowngraded);
  const tier = useAppStore(s => s.tier);
  const recoverLicenseByEmail = useAppStore(s => s.recoverLicenseByEmail);
  const activateLicense = useAppStore(s => s.activateLicense);
  const licenseLoading = useAppStore(s => s.licenseLoading);

  const [email, setEmail] = useState('');
  const [licenseKey, setLicenseKey] = useState('');
  const [mode, setMode] = useState<'email' | 'key'>('email');
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);

  if (!wasDowngraded || tier !== 'free') return null;
  if (success) return null;

  const handleEmailRecover = async () => {
    if (!email.trim() || !email.includes('@')) {
      setError(t('license.recovery.invalidEmail', 'Enter a valid email address'));
      return;
    }
    setError('');
    const result = await recoverLicenseByEmail(email.trim());
    if (result.ok) {
      setSuccess(true);
    } else if (result.reason === 'not_found') {
      setError(t('license.recovery.notFound', 'No license found for this email. Try the email you used at checkout.'));
    } else {
      setError(result.reason ?? t('license.recovery.failed', 'Recovery failed. Check your connection and try again.'));
    }
  };

  const handleKeyActivate = async () => {
    if (!licenseKey.trim()) {
      setError(t('license.recovery.emptyKey', 'Paste your license key'));
      return;
    }
    setError('');
    const result = await activateLicense(licenseKey.trim());
    if (result.ok) {
      setSuccess(true);
    } else {
      setError(result.reason ?? t('license.recovery.invalidKey', 'Invalid license key'));
    }
  };

  return (
    <div className="mx-4 mt-2 mb-1 bg-red-500/8 border border-red-500/30 rounded-lg overflow-hidden">
      <div className="px-3 py-2">
        <div className="flex items-center gap-2 mb-2">
          <div className="w-2 h-2 rounded-full bg-red-400 animate-pulse" />
          <span className="text-sm font-medium text-red-400">
            {t('license.recovery.title', 'Signal features deactivated')}
          </span>
        </div>
        <p className="text-xs text-text-secondary mb-3">
          {t('license.recovery.description', 'Your license key could not be verified. Recover access using your purchase email or re-enter your key.')}
        </p>

        <div className="flex gap-2 mb-2">
          <button
            onClick={() => { setMode('email'); setError(''); }}
            className={`px-2 py-1 text-xs rounded transition-colors ${
              mode === 'email'
                ? 'bg-white/10 text-text-primary'
                : 'text-text-muted hover:text-text-secondary'
            }`}
          >
            {t('license.recovery.byEmail', 'Recover by email')}
          </button>
          <button
            onClick={() => { setMode('key'); setError(''); }}
            className={`px-2 py-1 text-xs rounded transition-colors ${
              mode === 'key'
                ? 'bg-white/10 text-text-primary'
                : 'text-text-muted hover:text-text-secondary'
            }`}
          >
            {t('license.recovery.byKey', 'Enter key')}
          </button>
        </div>

        {mode === 'email' ? (
          <div className="flex gap-2">
            <input
              type="email"
              value={email}
              onChange={e => setEmail(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') void handleEmailRecover(); }}
              placeholder={t('license.recovery.emailPlaceholder', 'Purchase email address')}
              className="flex-1 px-2 py-1.5 text-xs bg-bg-tertiary border border-border rounded text-text-primary placeholder:text-text-muted focus:outline-none focus:border-white/30"
              disabled={licenseLoading}
            />
            <button
              onClick={() => void handleEmailRecover()}
              disabled={licenseLoading}
              className="px-3 py-1.5 text-xs rounded bg-white/10 text-text-primary hover:bg-white/15 transition-colors disabled:opacity-50"
            >
              {licenseLoading
                ? t('license.recovery.recovering', 'Recovering...')
                : t('license.recovery.recover', 'Recover')}
            </button>
          </div>
        ) : (
          <div className="flex gap-2">
            <input
              type="text"
              value={licenseKey}
              onChange={e => setLicenseKey(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') void handleKeyActivate(); }}
              placeholder={t('license.recovery.keyPlaceholder', 'Paste license key (4DA-... or BE3529-...)')}
              className="flex-1 px-2 py-1.5 text-xs bg-bg-tertiary border border-border rounded text-text-primary placeholder:text-text-muted focus:outline-none focus:border-white/30 font-mono"
              disabled={licenseLoading}
            />
            <button
              onClick={() => void handleKeyActivate()}
              disabled={licenseLoading}
              className="px-3 py-1.5 text-xs rounded bg-white/10 text-text-primary hover:bg-white/15 transition-colors disabled:opacity-50"
            >
              {licenseLoading
                ? t('license.recovery.activating', 'Activating...')
                : t('license.recovery.activate', 'Activate')}
            </button>
          </div>
        )}

        {error && (
          <p className="text-xs text-red-400 mt-1.5">{error}</p>
        )}
      </div>
    </div>
  );
}
