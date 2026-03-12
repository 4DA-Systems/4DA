import { useTranslation } from 'react-i18next';

/**
 * Shared signal feed for team dashboard.
 *
 * This component will display signals shared by team members via the relay.
 * For now it shows a placeholder — the data layer will be wired when
 * team_monitoring queries are connected to the frontend store.
 *
 * Future data source: team_signals table + TeamSignal type
 */
export function TeamSignalFeed() {
  const { t } = useTranslation();

  // TODO: Wire to store when team signal query commands are added
  // const teamSignals = useAppStore(s => s.teamSignals);

  return (
    <div className="space-y-2">
      <p className="text-xs text-text-muted text-center py-6">
        {t('team.signals.empty', 'No shared signals yet. Team signals appear here when members share discoveries.')}
      </p>
      <p className="text-[10px] text-text-muted text-center">
        {t('team.signals.hint', 'Share signals from your Signal Chains view, or enable auto-sharing in Team Settings.')}
      </p>
    </div>
  );
}
