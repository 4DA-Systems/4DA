import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

export const ShowAllViewsToggle = memo(function ShowAllViewsToggle() {
  const { t } = useTranslation();
  const showAllViews = useAppStore(s => s.showAllViews);
  const setShowAllViews = useAppStore(s => s.setShowAllViews);

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-center justify-between">
        <div>
          <span className="text-white text-sm font-medium">{t('settings.display.showAllViews')}</span>
          <p className="text-text-muted text-xs mt-0.5">{t('settings.display.showAllViewsDescription')}</p>
        </div>
        <button
          onClick={() => setShowAllViews(!showAllViews)}
          role="switch"
          aria-checked={showAllViews}
          aria-label={t('settings.display.showAllViews')}
          className={`relative w-10 h-5 rounded-full transition-colors ${
            showAllViews ? 'bg-green-500/40' : 'bg-gray-600'
          }`}
        >
          <span className={`absolute top-0.5 start-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
            showAllViews ? 'translate-x-5' : 'translate-x-0'
          }`} />
        </button>
      </div>
    </div>
  );
});
