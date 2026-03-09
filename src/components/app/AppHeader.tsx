import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { VoidEngine } from '../void-engine/VoidEngine';
import { OllamaStatus } from '../OllamaStatus';

interface AppHeaderProps {
  monitoring: { enabled: boolean } | null;
  settingsFormProvider: string;
  isPro: boolean;
  tier: string;
  onOpenSettings: () => void;
  proValueBadge: React.ReactNode;
}

export const AppHeader = memo(function AppHeader({
  monitoring,
  settingsFormProvider,
  isPro,
  tier,
  onOpenSettings,
  proValueBadge,
}: AppHeaderProps) {
  const { t } = useTranslation();

  return (
    <header className="mb-8 flex items-center justify-between">
      <div className="flex items-center gap-4">
        <div className="w-12 h-12 flex items-center justify-center flex-shrink-0">
          <VoidEngine size={48} />
        </div>
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-white">{t('app.title')}</h1>
          <p className="text-gray-500 text-sm">{t('app.tagline')}</p>
        </div>
      </div>
      <div className="flex items-center gap-3">
        {monitoring?.enabled && (
          <div className="flex items-center gap-2 px-3 py-2 bg-green-500/10 border border-green-500/30 rounded-lg">
            <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
            <span className="text-xs text-green-400 font-medium">{t('header.live')}</span>
          </div>
        )}
        <OllamaStatus provider={settingsFormProvider} />
        {proValueBadge}
        <span className={`px-2 py-1 text-[10px] font-bold uppercase tracking-wider rounded ${
          isPro
            ? 'bg-[#D4AF37]/20 text-[#D4AF37] border border-[#D4AF37]/30'
            : 'bg-bg-tertiary text-gray-500 border border-border'
        }`}>
          {tier}
        </span>
        <button
          data-settings-trigger
          onClick={onOpenSettings}
          className="px-4 py-2 text-sm bg-bg-secondary text-gray-300 border border-border rounded-lg hover:bg-bg-tertiary hover:border-orange-500/30 transition-all"
        >
          {t('header.settings')}
        </button>
      </div>
    </header>
  );
});
