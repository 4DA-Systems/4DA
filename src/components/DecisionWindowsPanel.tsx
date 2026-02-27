import { useEffect, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import type { DecisionWindow } from '../types/autophagy';

const WINDOW_TYPE_CONFIG: Record<string, { label: string; color: string; border: string; bg: string }> = {
  security_patch: { label: 'Security', color: 'text-red-400', border: 'border-red-500/30', bg: 'bg-red-500/10' },
  migration: { label: 'Migration', color: 'text-amber-400', border: 'border-amber-500/30', bg: 'bg-amber-500/10' },
  adoption: { label: 'Adoption', color: 'text-blue-400', border: 'border-blue-500/30', bg: 'bg-blue-500/10' },
  knowledge: { label: 'Knowledge', color: 'text-purple-400', border: 'border-purple-500/30', bg: 'bg-purple-500/10' },
};

function getTimeRemaining(expiresAt: string | null): string | null {
  if (!expiresAt) return null;
  const now = Date.now();
  const exp = new Date(expiresAt).getTime();
  const diff = exp - now;
  if (diff <= 0) return 'Expired';
  const hours = Math.floor(diff / (1000 * 60 * 60));
  const days = Math.floor(hours / 24);
  if (days > 0) return `${days}d ${hours % 24}h left`;
  if (hours > 0) return `${hours}h left`;
  return `${Math.floor(diff / (1000 * 60))}m left`;
}

function UrgencyBar({ urgency }: { urgency: number }) {
  const pct = Math.round(urgency * 100);
  const color = urgency >= 0.8 ? 'bg-red-400' : urgency >= 0.5 ? 'bg-amber-400' : 'bg-blue-400';
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-1 bg-bg-tertiary rounded-full overflow-hidden">
        <div className={`h-full rounded-full transition-all ${color}`} style={{ width: `${pct}%` }} />
      </div>
      <span className="text-[10px] text-gray-500 tabular-nums">{pct}%</span>
    </div>
  );
}

const WindowCard = memo(function WindowCard({
  window,
  onAct,
  onDismiss,
}: {
  window: DecisionWindow;
  onAct: (id: number) => void;
  onDismiss: (id: number) => void;
}) {
  const { t } = useTranslation();
  const config = WINDOW_TYPE_CONFIG[window.window_type] ?? WINDOW_TYPE_CONFIG.knowledge;
  const timeLeft = getTimeRemaining(window.expires_at);

  return (
    <div className={`bg-bg-secondary rounded-lg border ${config.border} p-4`}>
      <div className="flex items-start justify-between gap-3 mb-2">
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${config.color} ${config.bg}`}>
            {config.label}
          </span>
          {window.streets_engine && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-[#D4AF37]/10 text-[#D4AF37] border border-[#D4AF37]/20 font-medium">
              {window.streets_engine}
            </span>
          )}
        </div>
        {timeLeft && (
          <span className="text-[10px] text-gray-500 flex-shrink-0">{timeLeft}</span>
        )}
      </div>
      <h4 className="text-sm text-white font-medium mb-1 truncate">{window.title}</h4>
      {window.description && (
        <p className="text-xs text-gray-400 mb-2 line-clamp-2">{window.description}</p>
      )}
      {window.dependency && (
        <div className="text-[10px] text-gray-500 mb-2">
          {t('decisions.affects')}: <span className="text-gray-300 font-mono">{window.dependency}</span>
        </div>
      )}
      <UrgencyBar urgency={window.urgency} />
      <div className="flex items-center gap-2 mt-3">
        <button
          onClick={() => onAct(window.id)}
          className={`flex-1 px-3 py-1.5 text-xs font-medium rounded-lg transition-colors ${config.color} ${config.bg} hover:opacity-80`}
        >
          {t('decisions.act')}
        </button>
        <button
          onClick={() => onDismiss(window.id)}
          className="px-3 py-1.5 text-xs text-gray-500 bg-bg-tertiary rounded-lg hover:text-gray-300 hover:bg-[#2A2A2A] transition-colors"
        >
          {t('action.dismiss')}
        </button>
      </div>
    </div>
  );
});

export const DecisionWindowsPanel = memo(function DecisionWindowsPanel() {
  const { t } = useTranslation();
  const windows = useAppStore(s => s.decisionWindows);
  const loading = useAppStore(s => s.decisionWindowsLoading);
  const loadWindows = useAppStore(s => s.loadDecisionWindows);
  const actOnWindow = useAppStore(s => s.actOnWindow);
  const closeWindow = useAppStore(s => s.closeWindow);

  useEffect(() => {
    loadWindows();
  }, [loadWindows]);

  const openWindows = useMemo(
    () => (windows ?? []).filter(w => w.status === 'open').sort((a, b) => b.urgency - a.urgency),
    [windows],
  );

  if (loading && openWindows.length === 0) return null;
  if (openWindows.length === 0) return null;

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium text-white">{t('decisions.title')}</h3>
          <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-bg-tertiary text-gray-400">
            {openWindows.length}
          </span>
        </div>
        <span className="text-[10px] text-gray-500">{t('decisions.subtitle')}</span>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {openWindows.map(w => (
          <WindowCard
            key={w.id}
            window={w}
            onAct={actOnWindow}
            onDismiss={closeWindow}
          />
        ))}
      </div>
    </div>
  );
});
