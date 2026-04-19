// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { reportError } from '../../lib/error-reporter';

interface DigestSectionProps {
  setSettingsStatus: (status: string) => void;
}

interface DigestConfig {
  enabled: boolean;
  save_local: boolean;
  frequency: string;
  min_score: number;
  max_items: number;
}

interface SmtpForm {
  email: string;
  host: string;
  port: string;
  username: string;
  password: string;
  fromAddress: string;
  useTls: boolean;
}

const SMTP_PRESETS: Record<string, { host: string; port: string }> = {
  'gmail.com': { host: 'smtp.gmail.com', port: '587' },
  'outlook.com': { host: 'smtp-mail.outlook.com', port: '587' },
  'hotmail.com': { host: 'smtp-mail.outlook.com', port: '587' },
  'yahoo.com': { host: 'smtp.mail.yahoo.com', port: '587' },
  'icloud.com': { host: 'smtp.mail.me.com', port: '587' },
  'protonmail.com': { host: 'smtp.protonmail.ch', port: '587' },
};

export function DigestSection({ setSettingsStatus }: DigestSectionProps) {
  const { t } = useTranslation();
  const [digestConfig, setDigestConfig] = useState<DigestConfig | null>(null);
  const [emailEnabled, setEmailEnabled] = useState(false);
  const [smtpForm, setSmtpForm] = useState<SmtpForm>({
    email: '',
    host: '',
    port: '587',
    username: '',
    password: '',
    fromAddress: '',
    useTls: true,
  });
  const [testingSend, setTestingSend] = useState(false);
  const [savingSmtp, setSavingSmtp] = useState(false);

  useEffect(() => {
    loadDigestConfig();
  }, []);

  const loadDigestConfig = async () => {
    try {
      const config = await cmd('get_digest_config') as unknown as DigestConfig;
      setDigestConfig(config);
    } catch (error) {
      reportError('DigestSection.loadConfig', error);
    }
  };

  const handleToggleDigest = async () => {
    if (!digestConfig) return;
    try {
      await cmd('set_digest_config', {
        enabled: !digestConfig.enabled,
      });
      setDigestConfig({ ...digestConfig, enabled: !digestConfig.enabled });
      setSettingsStatus(digestConfig.enabled ? t('settings.digest.disabled') : t('settings.digest.enabled'));
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  const handleEmailChange = useCallback((value: string) => {
    setSmtpForm(prev => {
      const next = { ...prev, email: value };
      // Auto-detect SMTP preset from email domain
      const domain = value.split('@')[1]?.toLowerCase();
      if (domain && SMTP_PRESETS[domain]) {
        next.host = SMTP_PRESETS[domain].host;
        next.port = SMTP_PRESETS[domain].port;
        if (!next.username) next.username = value;
        if (!next.fromAddress) next.fromAddress = value;
      }
      return next;
    });
  }, []);

  const handleSaveSmtp = async () => {
    setSavingSmtp(true);
    try {
      await cmd('set_digest_email_config', {
        email: smtpForm.email || undefined,
        smtp_host: smtpForm.host || undefined,
        smtp_port: smtpForm.port ? parseInt(smtpForm.port, 10) : undefined,
        smtp_username: smtpForm.username || undefined,
        smtp_password: smtpForm.password || undefined,
        smtp_from: smtpForm.fromAddress || undefined,
        smtp_use_tls: smtpForm.useTls,
      });
      setSettingsStatus(t('settings.digest.emailConfigSavedSessionOnly', 'Email configuration saved (password is session-only and will need to be re-entered after restart)'));
      setTimeout(() => setSettingsStatus(''), 4000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    } finally {
      setSavingSmtp(false);
    }
  };

  const handleTestEmail = async () => {
    setTestingSend(true);
    try {
      // Save first, then test
      await handleSaveSmtp();
      const result = await cmd('test_digest_email');
      setSettingsStatus(result);
      setTimeout(() => setSettingsStatus(''), 4000);
    } catch (error) {
      setSettingsStatus(`${t('settings.digest.emailTestFailed')}: ${error}`);
    } finally {
      setTestingSend(false);
    }
  };

  const smtpConfigured = smtpForm.email && smtpForm.host && smtpForm.username && smtpForm.password;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-start gap-3 mb-3">
        <div className="w-8 h-8 bg-purple-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-purple-400">&#x1f4cb;</span>
        </div>
        <div>
          <h3 className="text-white font-medium">{t('settings.digest.title')}</h3>
          <p className="text-text-muted text-sm mt-1">
            {t('settings.digest.description')}
          </p>
        </div>
      </div>

      {digestConfig ? (
        <div className="space-y-3">
          <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="flex items-center gap-3">
              <div className={`w-2 h-2 rounded-full ${digestConfig.enabled ? 'bg-green-500' : 'bg-gray-500'}`} />
              <span className="text-sm text-text-secondary">
                {digestConfig.enabled ? t('status.active') : t('status.inactive')}
              </span>
            </div>
            <button
              onClick={handleToggleDigest}
              className={`px-4 py-2 text-sm rounded-lg transition-all ${
                digestConfig.enabled
                  ? 'bg-red-500/10 text-red-400 border border-red-500/30 hover:bg-red-500/20'
                  : 'bg-green-500/10 text-green-400 border border-green-500/30 hover:bg-green-500/20'
              }`}
            >
              {digestConfig.enabled ? t('action.disable') : t('action.enable')}
            </button>
          </div>

          {digestConfig.enabled && (
            <>
              <div className="grid grid-cols-3 gap-3">
                <div className="p-3 bg-bg-secondary rounded-lg border border-border text-center">
                  <div className="text-xs text-text-muted mb-1">{t('settings.digest.frequency')}</div>
                  <div className="text-sm text-white font-medium">{digestConfig.frequency}</div>
                </div>
                <div className="p-3 bg-bg-secondary rounded-lg border border-border text-center">
                  <div className="text-xs text-text-muted mb-1">{t('settings.digest.minScore')}</div>
                  <div className="text-sm text-white font-medium">{Math.round(digestConfig.min_score * 100)}%</div>
                </div>
                <div className="p-3 bg-bg-secondary rounded-lg border border-border text-center">
                  <div className="text-xs text-text-muted mb-1">{t('settings.digest.maxItems')}</div>
                  <div className="text-sm text-white font-medium">{digestConfig.max_items}</div>
                </div>
              </div>

              {/* Email Delivery — opt-in */}
              <div className="border border-border rounded-lg overflow-hidden">
                <button
                  onClick={() => setEmailEnabled(!emailEnabled)}
                  className="w-full flex items-center justify-between p-3 bg-bg-secondary hover:bg-bg-tertiary transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <div className={`w-2 h-2 rounded-full ${emailEnabled ? 'bg-blue-500' : 'bg-gray-600'}`} />
                    <span className="text-sm text-text-secondary">{t('settings.digest.emailDelivery')}</span>
                    <span className="text-xs text-text-muted">{t('settings.digest.emailDeliveryOptional')}</span>
                  </div>
                  <svg
                    className={`w-4 h-4 text-text-muted transition-transform ${emailEnabled ? 'rotate-180' : ''}`}
                    fill="none" viewBox="0 0 24 24" stroke="currentColor"
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                  </svg>
                </button>

                {emailEnabled && (
                  <div className="p-4 space-y-4 border-t border-border">
                    <p className="text-xs text-text-muted leading-relaxed">
                      {t('settings.digest.privacyNote')}
                    </p>

                    <div className="space-y-3">
                      <div>
                        <label className="block text-xs text-text-muted mb-1">{t('settings.digest.emailAddress')}</label>
                        <input
                          type="email"
                          value={smtpForm.email}
                          onChange={e => handleEmailChange(e.target.value)}
                          placeholder="you@example.com"
                          className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-text-muted focus:outline-none focus:border-blue-500/50"
                        />
                      </div>

                      <div className="grid grid-cols-2 gap-3">
                        <div>
                          <label className="block text-xs text-text-muted mb-1">{t('settings.digest.smtpHost')}</label>
                          <input
                            type="text"
                            value={smtpForm.host}
                            onChange={e => setSmtpForm(prev => ({ ...prev, host: e.target.value }))}
                            placeholder="smtp.gmail.com"
                            className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-text-muted focus:outline-none focus:border-blue-500/50"
                          />
                        </div>
                        <div>
                          <label className="block text-xs text-text-muted mb-1">{t('settings.digest.port')}</label>
                          <input
                            type="text"
                            value={smtpForm.port}
                            onChange={e => setSmtpForm(prev => ({ ...prev, port: e.target.value }))}
                            placeholder="587"
                            className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-text-muted focus:outline-none focus:border-blue-500/50"
                          />
                        </div>
                      </div>

                      <div>
                        <label className="block text-xs text-text-muted mb-1">{t('settings.digest.username')}</label>
                        <input
                          type="text"
                          value={smtpForm.username}
                          onChange={e => setSmtpForm(prev => ({ ...prev, username: e.target.value }))}
                          placeholder="you@example.com"
                          className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-text-muted focus:outline-none focus:border-blue-500/50"
                        />
                      </div>

                      <div>
                        <label className="block text-xs text-text-muted mb-1">{t('settings.digest.password')}</label>
                        <input
                          type="password"
                          value={smtpForm.password}
                          onChange={e => setSmtpForm(prev => ({ ...prev, password: e.target.value }))}
                          placeholder="App password or SMTP password"
                          className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-text-muted focus:outline-none focus:border-blue-500/50"
                        />
                        <p className="text-xs text-text-muted mt-1 opacity-70">
                          {t('settings.digest.passwordSessionOnly', 'Session-only — not persisted to disk for security')}
                        </p>
                      </div>

                      <div>
                        <label className="block text-xs text-text-muted mb-1">{t('settings.digest.fromAddress')}</label>
                        <input
                          type="email"
                          value={smtpForm.fromAddress}
                          onChange={e => setSmtpForm(prev => ({ ...prev, fromAddress: e.target.value }))}
                          placeholder="you@example.com"
                          className="w-full bg-bg-primary border border-border rounded-lg px-3 py-2 text-sm text-white placeholder-text-muted focus:outline-none focus:border-blue-500/50"
                        />
                      </div>

                      <label className="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
                        <input
                          type="checkbox"
                          checked={smtpForm.useTls}
                          onChange={e => setSmtpForm(prev => ({ ...prev, useTls: e.target.checked }))}
                          className="rounded border-border"
                        />
                        {t('settings.digest.useTls')}
                      </label>
                    </div>

                    <div className="flex gap-3 pt-2">
                      <button
                        onClick={handleSaveSmtp}
                        disabled={savingSmtp || !smtpForm.email}
                        className="px-4 py-2 text-sm rounded-lg bg-white/10 text-white border border-border hover:bg-white/20 transition-colors disabled:opacity-40"
                      >
                        {savingSmtp ? t('action.saving') : t('settings.digest.saveSmtp')}
                      </button>
                      <button
                        onClick={handleTestEmail}
                        disabled={testingSend || !smtpConfigured}
                        className="px-4 py-2 text-sm rounded-lg bg-blue-500/10 text-blue-400 border border-blue-500/30 hover:bg-blue-500/20 transition-colors disabled:opacity-40"
                      >
                        {testingSend ? t('settings.digest.sendingTest') : t('settings.digest.sendTestEmail')}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      ) : (
        <div className="text-sm text-text-muted">{t('settings.digest.loading')}</div>
      )}
    </div>
  );
}
