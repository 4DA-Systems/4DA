import { memo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

interface StreetsMembershipSectionProps {
  onStatus: (s: string) => void;
}

export const StreetsMembershipSection = memo(function StreetsMembershipSection({ onStatus: _onStatus }: StreetsMembershipSectionProps) {
  const { t } = useTranslation();
  const loadStreetsTier = useAppStore(s => s.loadStreetsTier);

  useEffect(() => { loadStreetsTier(); }, [loadStreetsTier]);

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-3">{t('settings.streets.title')}</h3>
      <div className="flex items-center gap-2">
        <span className="text-xs text-text-muted">{t('settings.streets.currentTier')}</span>
        <span className="text-xs font-semibold text-text-secondary">{t('settings.streets.tierPlaybook')}</span>
      </div>
    </div>
  );
});
