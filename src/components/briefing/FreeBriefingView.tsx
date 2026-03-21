import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { EngagementPulse } from '../EngagementPulse';
import type { FreeBriefingData } from '../../store/types';

interface FreeBriefingViewProps {
  freeBriefing: FreeBriefingData;
  generateBriefing: () => Promise<void>;
}

export const FreeBriefingView = memo(function FreeBriefingView({
  freeBriefing,
  generateBriefing,
}: FreeBriefingViewProps) {
  const { t } = useTranslation();

  return (
    <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg space-y-4">
      <div className="bg-bg-secondary rounded-lg border border-border p-5">
        <h2 className="font-medium text-white mb-3">{t('briefing.dailyOverview')}</h2>
        <div className="space-y-3">
          {freeBriefing.top_items?.map((item, i) => (
            <div key={i} className="flex items-start gap-3">
              <span className="text-xs text-orange-400 font-mono font-medium flex-shrink-0 mt-0.5">{item.score}</span>
              <div className="min-w-0">
                {item.url ? (
                  <button
                    onClick={() => window.open(item.url!, '_blank', 'noopener,noreferrer')}
                    className="text-sm text-white hover:text-orange-400 text-left transition-colors"
                  >
                    {item.title}
                  </button>
                ) : (
                  <span className="text-sm text-white">{item.title}</span>
                )}
                <span className="text-xs text-text-muted ml-2">{item.source}</span>
              </div>
            </div>
          ))}
        </div>
        {freeBriefing.stack_alerts && freeBriefing.stack_alerts.length > 0 && (
          <div className="mt-4 pt-3 border-t border-border">
            <h3 className="text-xs font-medium text-amber-400 mb-2">{t('briefing.stackAlerts')}</h3>
            {freeBriefing.stack_alerts.map((alert, i) => (
              <div key={i} className="text-xs text-text-secondary py-0.5">{alert.title}</div>
            ))}
          </div>
        )}
        <div className="mt-3 pt-3 border-t border-border flex items-center justify-between">
          <span className="text-xs text-text-muted">{t('briefing.itemsAnalyzed', { count: freeBriefing.total_items })}</span>
          <button
            onClick={generateBriefing}
            className="px-3 py-1.5 text-xs bg-orange-500/10 text-orange-400 border border-orange-500/20 rounded-lg hover:bg-orange-500/20 transition-all font-medium"
          >
            {t('briefing.generateAI')}
          </button>
        </div>
      </div>
      <EngagementPulse />
    </section>
  );
});
