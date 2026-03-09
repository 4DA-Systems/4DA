import { memo, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import type { StreetsTier } from '../../store/playbook-slice';

interface StreetsMembershipSectionProps {
  onStatus: (s: string) => void;
}

export const StreetsMembershipSection = memo(function StreetsMembershipSection({ onStatus }: StreetsMembershipSectionProps) {
  const { t } = useTranslation();
  const streetsTier = useAppStore(s => s.streetsTier);
  const activateStreetsLicense = useAppStore(s => s.activateStreetsLicense);
  const loadStreetsTier = useAppStore(s => s.loadStreetsTier);
  const [key, setKey] = useState('');
  const [activating, setActivating] = useState(false);

  useEffect(() => { loadStreetsTier(); }, [loadStreetsTier]);

  const tierLabels: Record<StreetsTier, { label: string; color: string }> = {
    playbook: { label: t('settings.streets.tierPlaybook'), color: 'text-text-secondary' },
    community: { label: t('settings.streets.tierCommunity'), color: 'text-[#D4AF37]' },
    cohort: { label: t('settings.streets.tierCohort'), color: 'text-[#22C55E]' },
  };

  const { label, color } = tierLabels[streetsTier] || tierLabels.playbook;

  const handleActivate = async () => {
    if (!key.trim()) return;
    setActivating(true);
    const ok = await activateStreetsLicense(key.trim());
    setActivating(false);
    if (ok) {
      onStatus(t('settings.streets.activated'));
      setKey('');
      setTimeout(() => onStatus(''), 3000);
    } else {
      onStatus(t('settings.streets.invalidKey'));
      setTimeout(() => onStatus(''), 3000);
    }
  };

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-3">{t('settings.streets.title')}</h3>
      <div className="flex items-center gap-2 mb-3">
        <span className="text-xs text-text-muted">{t('settings.streets.currentTier')}</span>
        <span className={`text-xs font-semibold ${color}`}>{label}</span>
      </div>
      {streetsTier === 'playbook' && (
        <div className="flex gap-2">
          <input
            type="text"
            value={key}
            onChange={e => setKey(e.target.value)}
            placeholder={t('settings.streets.placeholder')}
            className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder-gray-600 focus:outline-none focus:border-[#D4AF37]/50"
          />
          <button
            onClick={handleActivate}
            disabled={activating || !key.trim()}
            className="px-4 py-2 text-sm font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50"
          >
            {activating ? '...' : t('action.activate')}
          </button>
        </div>
      )}
    </div>
  );
});
