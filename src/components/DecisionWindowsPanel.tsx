import { useEffect, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranslatedContent } from './ContentTranslationProvider';
import { useAppStore } from '../store';
import { registerGameComponent } from '../lib/game-components';
import type { DecisionWindow } from '../types/autophagy';

const WINDOW_TYPE_CONFIG: Record<string, { label: string; color: string; border: string; bg: string }> = {
  security_patch: { label: 'Security', color: 'text-red-400', border: 'border-red-500/30', bg: 'bg-red-500/10' },
  migration: { label: 'Migration', color: 'text-amber-400', border: 'border-amber-500/30', bg: 'bg-amber-500/10' },
  adoption: { label: 'Adoption', color: 'text-blue-400', border: 'border-blue-500/30', bg: 'bg-blue-500/10' },
  knowledge: { label: 'Knowledge', color: 'text-purple-400', border: 'border-purple-500/30', bg: 'bg-purple-500/10' },
};

function getTimeRemaining(expiresAt: string | null, t: (key: string, opts?: Record<string, unknown>) => string): string | null {
  if (!expiresAt) return null;
  const now = Date.now();
  const exp = new Date(expiresAt).getTime();
  const diff = exp - now;
  if (diff <= 0) return t('decisions.expired');
  const hours = Math.floor(diff / (1000 * 60 * 60));
  const days = Math.floor(hours / 24);
  if (days > 0) return t('decisions.timeLeftDaysHours', { days, hours: hours % 24 });
  if (hours > 0) return t('decisions.timeLeftHours', { hours });
  return t('decisions.timeLeftMinutes', { minutes: Math.floor(diff / (1000 * 60)) });
}

function UrgencyBar({ urgency }: { urgency: number }) {
  const pct = Math.round(urgency * 100);
  const color = urgency >= 0.8 ? 'bg-red-400' : urgency >= 0.5 ? 'bg-amber-400' : 'bg-blue-400';
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-1 bg-bg-tertiary rounded-full overflow-hidden">
        <div className={`h-full rounded-full transition-all ${color}`} style={{ width: `${pct}%` }} />
      </div>
      <span className="text-[10px] text-text-muted tabular-nums">{pct}%</span>
    </div>
  );
}

const WindowCard = memo(function WindowCard({
  window,
  onAct,
  onDismiss,
  index = 0,
}: {
  window: DecisionWindow;
  onAct: (id: number) => void;
  onDismiss: (id: number) => void;
  /** Card position for staggered animation delay */
  index?: number;
}) {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();
  const config = (WINDOW_TYPE_CONFIG[window.window_type] ?? WINDOW_TYPE_CONFIG.knowledge)!;
  const timeLeft = getTimeRemaining(window.expires_at, t);

  return (
    <div
      className={`bg-bg-secondary rounded-lg border ${config.border} p-4`}
      style={{
        animation: 'slideInRight 0.4s ease-out both',
        animationDelay: `${index * 80}ms`,
      }}
    >
      <div className="flex items-start justify-between gap-3 mb-2">
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${config.color} ${config.bg}`}>
            {config.label}
          </span>
          {window.streets_engine && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-accent-gold/10 text-accent-gold border border-accent-gold/20 font-medium">
              {window.streets_engine}
            </span>
          )}
        </div>
        {timeLeft && (
          <div className="flex items-center gap-1.5 flex-shrink-0">
            <div className="w-4 h-4 overflow-hidden">
              <game-decision-countdown style={{ width: '16px', height: '16px' }} aria-hidden="true" />
            </div>
            <span className="text-[10px] text-text-muted">{timeLeft}</span>
          </div>
        )}
      </div>
      <h4 className="text-sm text-white font-medium mb-1 truncate">{getTranslated(`dw-title-${window.id}`, window.title)}</h4>
      {window.description && (
        <p className="text-xs text-text-secondary mb-2 line-clamp-2">{getTranslated(`dw-desc-${window.id}`, window.description)}</p>
      )}
      {window.dependency && (
        <div className="text-[10px] text-text-muted mb-2">
          {t('decisions.affects')}: <span className="text-text-secondary font-mono">{window.dependency}</span>
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
          className="px-3 py-1.5 text-xs text-text-muted bg-bg-tertiary rounded-lg hover:text-text-secondary hover:bg-border transition-colors"
        >
          {t('action.dismiss')}
        </button>
      </div>
    </div>
  );
});

export const DecisionWindowsPanel = memo(function DecisionWindowsPanel() {
  const { t } = useTranslation();
  const { requestTranslation } = useTranslatedContent();
  const windows = useAppStore(s => s.decisionWindows);
  const loading = useAppStore(s => s.decisionWindowsLoading);
  const loadWindows = useAppStore(s => s.loadDecisionWindows);
  const actOnWindow = useAppStore(s => s.actOnWindow);
  const closeWindow = useAppStore(s => s.closeWindow);

  useEffect(() => {
    loadWindows();
  }, [loadWindows]);

  useEffect(() => { registerGameComponent('game-decision-countdown'); }, []);

  const openWindows = useMemo(
    () => (windows ?? []).filter(w => w.status === 'open').sort((a, b) => b.urgency - a.urgency),
    [windows],
  );

  // Request translations for decision window content
  useEffect(() => {
    if (openWindows.length > 0) {
      requestTranslation(openWindows.flatMap(w => {
        const items = [{ id: `dw-title-${w.id}`, text: w.title }];
        if (w.description) items.push({ id: `dw-desc-${w.id}`, text: w.description });
        return items;
      }));
    }
  }, [openWindows, requestTranslation]);

  if (loading && openWindows.length === 0) return null;
  if (openWindows.length === 0) return (
    <div className="bg-bg-secondary rounded-lg border border-border p-4">
      <p className="text-sm text-text-muted">{t('decisions.noWindows')}</p>
    </div>
  );

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium text-white">{t('decisions.title')}</h3>
          <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-bg-tertiary text-text-secondary">
            {openWindows.length}
          </span>
        </div>
        <span className="text-[10px] text-text-muted">{t('decisions.subtitle')}</span>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {openWindows.map((w, i) => (
          <WindowCard
            key={w.id}
            window={w}
            onAct={actOnWindow}
            onDismiss={closeWindow}
            index={i}
          />
        ))}
      </div>
    </div>
  );
});
