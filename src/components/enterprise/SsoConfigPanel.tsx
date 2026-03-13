import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface SsoConfig {
  provider_type: string;
  idp_url: string;
  entity_id: string;
  certificate: string | null;
  client_id: string | null;
  issuer: string | null;
  enabled: boolean;
}

interface SsoSession {
  email: string;
  display_name: string;
  groups: string[];
  authenticated_at: string;
  expires_at: string | null;
  provider_type: string;
}

export function SsoConfigPanel() {
  const { t } = useTranslation();

  const [config, setConfig] = useState<SsoConfig | null>(null);
  const [session, setSession] = useState<SsoSession | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  // Edit state
  const [editing, setEditing] = useState(false);
  const [form, setForm] = useState({
    provider_type: 'saml',
    idp_url: '',
    entity_id: 'com.4da.app',
    certificate: '',
    client_id: '',
    issuer: '',
    enabled: true,
  });

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [cfg, sess] = await Promise.all([
        cmd('get_sso_config').catch(() => null),
        cmd('get_sso_session').catch(() => null),
      ]);
      setConfig(cfg as SsoConfig | null);
      setSession(sess as SsoSession | null);
      if (cfg) {
        const c = cfg as SsoConfig;
        setForm({
          provider_type: c.provider_type,
          idp_url: c.idp_url,
          entity_id: c.entity_id,
          certificate: c.certificate || '',
          client_id: c.client_id || '',
          issuer: c.issuer || '',
          enabled: c.enabled,
        });
      }
    } catch { /* silent */ }
    setLoading(false);
  };

  const handleSave = async () => {
    setSaving(true);
    setStatus(null);
    try {
      await cmd('set_sso_config', { config: form });
      setConfig(form as unknown as SsoConfig);
      setEditing(false);
      setStatus({ ok: true, msg: 'SSO configuration saved' });
    } catch {
      setStatus({ ok: false, msg: 'Failed to save SSO configuration' });
    }
    setSaving(false);
    setTimeout(() => setStatus(null), 3000);
  };

  const handleLogin = async () => {
    try {
      const url = await cmd('initiate_sso_login');
      // Open the IdP login page in the system browser
      window.open(url as unknown as string, '_blank');
      setStatus({ ok: true, msg: 'SSO login initiated. Complete authentication in your browser.' });
    } catch {
      setStatus({ ok: false, msg: 'Failed to initiate SSO login' });
    }
    setTimeout(() => setStatus(null), 5000);
  };

  const handleLogout = async () => {
    try {
      await cmd('logout_sso');
      setSession(null);
      setStatus({ ok: true, msg: 'SSO session cleared' });
    } catch {
      setStatus({ ok: false, msg: 'Failed to logout' });
    }
    setTimeout(() => setStatus(null), 3000);
  };

  if (loading) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="animate-pulse space-y-3">
          <div className="h-4 bg-border rounded w-1/4" />
          <div className="h-16 bg-border rounded" />
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-white">
            {t('enterprise.sso.title', 'Single Sign-On (SSO)')}
          </h3>
          <p className="text-[10px] text-text-muted mt-0.5">
            {t('enterprise.sso.description', 'Configure SAML or OIDC authentication for your organization.')}
          </p>
        </div>
        <span className="text-[10px] px-1.5 py-0.5 bg-[#22C55E]/15 text-[#22C55E] rounded font-medium">
          Enterprise
        </span>
      </div>

      {/* Status */}
      {status && (
        <div className={`px-3 py-2 rounded text-xs ${
          status.ok ? 'bg-[#22C55E]/10 text-[#22C55E]' : 'bg-[#EF4444]/10 text-[#EF4444]'
        }`}>
          {status.msg}
        </div>
      )}

      {/* Active Session */}
      {session && (
        <div className="bg-bg-primary rounded-lg p-3 border border-[#22C55E]/30">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-xs text-white font-medium">{session.display_name}</p>
              <p className="text-[10px] text-text-muted">{session.email}</p>
              {session.groups.length > 0 && (
                <div className="flex items-center gap-1 mt-1">
                  {session.groups.map(g => (
                    <span key={g} className="text-[9px] px-1 py-0.5 bg-[#818CF8]/10 text-[#818CF8] rounded">
                      {g}
                    </span>
                  ))}
                </div>
              )}
            </div>
            <button
              onClick={handleLogout}
              className="text-[10px] text-text-muted hover:text-[#EF4444] transition-colors"
            >
              {t('enterprise.sso.logout', 'Sign Out')}
            </button>
          </div>
        </div>
      )}

      {/* SSO Config */}
      {editing ? (
        <div className="space-y-3">
          {/* Provider Type */}
          <div>
            <label className="text-[10px] text-text-muted block mb-1">Provider Type</label>
            <select
              value={form.provider_type}
              onChange={e => setForm(f => ({ ...f, provider_type: e.target.value }))}
              className="w-full px-2 py-1.5 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
            >
              <option value="saml">SAML 2.0</option>
              <option value="oidc">OpenID Connect</option>
            </select>
          </div>

          {/* IdP URL */}
          <div>
            <label className="text-[10px] text-text-muted block mb-1">Identity Provider URL</label>
            <input
              type="url"
              value={form.idp_url}
              onChange={e => setForm(f => ({ ...f, idp_url: e.target.value }))}
              className="w-full px-2 py-1.5 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
              placeholder="https://idp.example.com/saml/sso"
            />
          </div>

          {/* Entity ID */}
          <div>
            <label className="text-[10px] text-text-muted block mb-1">Entity ID / Audience</label>
            <input
              type="text"
              value={form.entity_id}
              onChange={e => setForm(f => ({ ...f, entity_id: e.target.value }))}
              className="w-full px-2 py-1.5 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
              placeholder="com.4da.app"
            />
          </div>

          {/* SAML Certificate */}
          {form.provider_type === 'saml' && (
            <div>
              <label className="text-[10px] text-text-muted block mb-1">IdP Certificate (PEM)</label>
              <textarea
                value={form.certificate}
                onChange={e => setForm(f => ({ ...f, certificate: e.target.value }))}
                rows={4}
                className="w-full px-2 py-1.5 text-[10px] font-mono bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50 resize-none"
                placeholder="-----BEGIN CERTIFICATE-----&#10;..."
              />
            </div>
          )}

          {/* OIDC fields */}
          {form.provider_type === 'oidc' && (
            <>
              <div>
                <label className="text-[10px] text-text-muted block mb-1">Client ID</label>
                <input
                  type="text"
                  value={form.client_id}
                  onChange={e => setForm(f => ({ ...f, client_id: e.target.value }))}
                  className="w-full px-2 py-1.5 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
                />
              </div>
              <div>
                <label className="text-[10px] text-text-muted block mb-1">Issuer URL</label>
                <input
                  type="url"
                  value={form.issuer}
                  onChange={e => setForm(f => ({ ...f, issuer: e.target.value }))}
                  className="w-full px-2 py-1.5 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
                  placeholder="https://idp.example.com"
                />
              </div>
            </>
          )}

          {/* Enable toggle */}
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={e => setForm(f => ({ ...f, enabled: e.target.checked }))}
              className="rounded border-border"
            />
            <span className="text-xs text-text-secondary">Enable SSO authentication</span>
          </label>

          {/* Actions */}
          <div className="flex items-center gap-2">
            <button
              onClick={handleSave}
              disabled={!form.idp_url.trim() || saving}
              className="px-3 py-1.5 text-xs bg-[#22C55E]/15 text-[#22C55E] rounded hover:bg-[#22C55E]/25 transition-colors disabled:opacity-50"
            >
              {saving ? t('action.saving', 'Saving...') : t('action.save', 'Save')}
            </button>
            <button
              onClick={() => setEditing(false)}
              className="px-3 py-1.5 text-xs text-text-muted hover:text-white transition-colors"
            >
              {t('action.cancel', 'Cancel')}
            </button>
          </div>
        </div>
      ) : (
        <div>
          {config ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between px-3 py-2.5 bg-bg-primary rounded-lg border border-border/50">
                <div>
                  <span className="text-xs text-white">
                    {config.provider_type.toUpperCase()} &mdash; {config.idp_url}
                  </span>
                  <div className="flex items-center gap-2 mt-0.5">
                    <div className={`w-1.5 h-1.5 rounded-full ${config.enabled ? 'bg-[#22C55E]' : 'bg-text-muted'}`} />
                    <span className="text-[10px] text-text-muted">
                      {config.enabled ? 'Active' : 'Disabled'}
                    </span>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {!session && config.enabled && (
                    <button
                      onClick={handleLogin}
                      className="text-[10px] px-2.5 py-1 bg-[#22C55E]/15 text-[#22C55E] rounded hover:bg-[#22C55E]/25 transition-colors"
                    >
                      {t('enterprise.sso.signIn', 'Sign In with SSO')}
                    </button>
                  )}
                  <button
                    onClick={() => setEditing(true)}
                    className="text-[10px] text-text-muted hover:text-text-secondary transition-colors"
                  >
                    {t('action.edit', 'Edit')}
                  </button>
                </div>
              </div>
            </div>
          ) : (
            <div className="text-center py-4">
              <p className="text-xs text-text-muted mb-3">
                {t('enterprise.sso.notConfigured', 'SSO is not configured. Set up SAML or OpenID Connect to authenticate team members.')}
              </p>
              <button
                onClick={() => setEditing(true)}
                className="px-4 py-2 text-xs bg-[#22C55E]/15 text-[#22C55E] rounded hover:bg-[#22C55E]/25 transition-colors"
              >
                {t('enterprise.sso.configure', 'Configure SSO')}
              </button>
            </div>
          )}
        </div>
      )}

      {/* Info */}
      <div className="px-3 py-2 rounded-lg bg-bg-primary border border-border/50">
        <p className="text-[9px] text-text-muted leading-relaxed">
          {t('enterprise.sso.info', 'SSO verification happens locally on your machine. 4DA never sends credentials to any server. Your identity provider authenticates directly to the desktop app via a localhost callback.')}
        </p>
      </div>
    </div>
  );
}
