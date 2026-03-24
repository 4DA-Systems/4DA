import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

const RESOURCE_TYPES: readonly { key: string; label: string; defaultDays: number }[] = [
  { key: 'audit_log', label: 'Audit Logs', defaultDays: 365 },
  { key: 'shared_resources', label: 'Shared Resources', defaultDays: 90 },
  { key: 'signals', label: 'Signals', defaultDays: 90 },
  { key: 'briefings', label: 'Briefings', defaultDays: 180 },
  { key: 'decisions', label: 'Decisions', defaultDays: 365 },
];

const RETENTION_OPTIONS = [
  { days: 30, label: '30 days' },
  { days: 60, label: '60 days' },
  { days: 90, label: '90 days' },
  { days: 180, label: '180 days' },
  { days: 365, label: '1 year' },
  { days: 0, label: 'Unlimited' },
];

export function PolicyEditor() {
  const { t } = useTranslation();
  const retentionPolicies = useAppStore(s => s.retentionPolicies);
  const loadRetentionPolicies = useAppStore(s => s.loadRetentionPolicies);
  const setRetentionPolicy = useAppStore(s => s.setRetentionPolicy);
  const orgLoading = useAppStore(s => s.orgLoading);

  const [saving, setSaving] = useState<string | null>(null);
  const [saveStatus, setSaveStatus] = useState<{ key: string; ok: boolean } | null>(null);

  useEffect(() => {
    loadRetentionPolicies();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const getRetentionDays = (resourceType: string, defaultDays: number): number => {
    const policy = retentionPolicies.find(p => p.resource_type === resourceType);
    return policy ? policy.retention_days : defaultDays;
  };

  const handlePolicyChange = async (resourceType: string, days: number) => {
    setSaving(resourceType);
    setSaveStatus(null);
    try {
      await setRetentionPolicy(resourceType, days);
      setSaveStatus({ key: resourceType, ok: true });
    } catch {
      setSaveStatus({ key: resourceType, ok: false });
    }
    setSaving(null);
    setTimeout(() => setSaveStatus(null), 2000);
  };

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-white">
            {t('enterprise.policies.title', 'Data Retention Policies')}
          </h3>
          <p className="text-[10px] text-text-muted mt-0.5">
            {t('enterprise.policies.description', 'Control how long different data types are retained. A 7-day grace period applies before deletion.')}
          </p>
        </div>
        <span className="text-[10px] px-1.5 py-0.5 bg-success/15 text-success rounded font-medium">
          {t('enterprise.org.enterprise', 'Enterprise')}
        </span>
      </div>

      {/* Retention Policy Table */}
      <div className="space-y-2">
        {RESOURCE_TYPES.map(({ key, label, defaultDays }) => {
          const currentDays = getRetentionDays(key, defaultDays);
          const isSaving = saving === key;
          const status = saveStatus?.key === key ? saveStatus : null;

          return (
            <div
              key={key}
              className="flex items-center justify-between px-3 py-2.5 bg-bg-primary rounded-lg border border-border/50"
            >
              <div className="flex-1">
                <span className="text-xs text-white">{label}</span>
                <span className="text-[10px] text-text-muted ms-2">
                  ({t('enterprise.policies.default', 'default')}: {defaultDays === 0 ? 'unlimited' : `${defaultDays}d`})
                </span>
              </div>

              <div className="flex items-center gap-2">
                {status && (
                  <span className={`text-[10px] ${status.ok ? 'text-success' : 'text-error'}`}>
                    {status.ok ? t('action.saved', 'Saved') : t('action.error', 'Error')}
                  </span>
                )}

                <select
                  value={currentDays}
                  onChange={e => handlePolicyChange(key, parseInt(e.target.value, 10))}
                  disabled={isSaving || orgLoading}
                  className="px-2 py-1 text-xs bg-bg-tertiary border border-border rounded text-white focus:outline-none focus:border-success/50 disabled:opacity-50"
                  aria-label={`Retention period for ${label}`}
                >
                  {RETENTION_OPTIONS.map(opt => (
                    <option key={opt.days} value={opt.days}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          );
        })}
      </div>

      {/* Info Box */}
      <div className="px-3 py-2.5 rounded-lg bg-bg-primary border border-border/50">
        <p className="text-[10px] text-text-muted leading-relaxed">
          {t('enterprise.policies.info',
            'Retention enforcement runs daily at 03:00 local time. Items within the 7-day grace period are flagged as "expiring soon" but not yet deleted. All cleanup operations are logged in the audit trail.'
          )}
        </p>
      </div>

      {/* Data Handling Summary */}
      <div>
        <h4 className="text-xs font-medium text-text-secondary mb-2">
          {t('enterprise.policies.dataHandling', 'Data Handling')}
        </h4>
        <div className="space-y-1.5 text-[10px] text-text-muted">
          <div className="flex items-start gap-2">
            <span className="text-success mt-0.5">&#8226;</span>
            <span>{t('enterprise.policies.noRawContent', 'No raw content is ever shared. Only metadata, scores, and embeddings.')}</span>
          </div>
          <div className="flex items-start gap-2">
            <span className="text-success mt-0.5">&#8226;</span>
            <span>{t('enterprise.policies.e2e', 'All team relay data is end-to-end encrypted (XChaCha20Poly1305).')}</span>
          </div>
          <div className="flex items-start gap-2">
            <span className="text-success mt-0.5">&#8226;</span>
            <span>{t('enterprise.policies.localFirst', 'Primary data storage remains local to each machine.')}</span>
          </div>
          <div className="flex items-start gap-2">
            <span className="text-success mt-0.5">&#8226;</span>
            <span>{t('enterprise.policies.relayDumb', 'The relay server cannot read encrypted payloads. "Dumb relay, smart clients."')}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
