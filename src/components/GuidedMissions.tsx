import { useState, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

interface MissionDef {
  id: number;
  key: string;
  target: number;
}

const MISSIONS: MissionDef[] = [
  { id: 1, key: 'saveArticles', target: 3 },
  { id: 2, key: 'dismissArticles', target: 2 },
  { id: 3, key: 'checkProfile', target: 1 },
  { id: 4, key: 'reviewNearMiss', target: 1 },
  { id: 5, key: 'autophagyCycle', target: 1 },
];

interface MissionState {
  current: number;
  completed: number[];
  dismissed: boolean;
  startedAt: string;
}

const STORAGE_KEY = '4da-guided-missions';

function loadMissionState(): MissionState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed.current === 'number') return parsed;
    }
  } catch { /* ignore */ }
  return { current: 1, completed: [], dismissed: false, startedAt: new Date().toISOString() };
}

function saveMissionState(state: MissionState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch { /* ignore */ }
}

/** Guided mission system for the first 48 hours. Non-intrusive card with progress. */
export const GuidedMissions = memo(function GuidedMissions() {
  const { t } = useTranslation();
  const [state, setState] = useState<MissionState>(loadMissionState);
  const feedbackGiven = useAppStore(s => s.feedbackGiven);
  const addToast = useAppStore(s => s.addToast);

  // Compute progress for current mission
  const currentMission = MISSIONS.find(m => m.id === state.current);
  const getProgress = useCallback((missionId: number): number => {
    const entries = Object.values(feedbackGiven);
    switch (missionId) {
      case 1: return entries.filter(a => a === 'save').length;
      case 2: return entries.filter(a => a === 'dismiss' || a === 'mark_irrelevant').length;
      case 3: return state.completed.includes(3) ? 1 : 0;
      case 4: return state.completed.includes(4) ? 1 : 0;
      case 5: return state.completed.includes(5) ? 1 : 0;
      default: return 0;
    }
  }, [feedbackGiven, state.completed]);

  // Check mission completion
  useEffect(() => {
    if (!currentMission || state.dismissed) return;
    const progress = getProgress(currentMission.id);
    if (progress >= currentMission.target && !state.completed.includes(currentMission.id)) {
      const nextState: MissionState = {
        ...state,
        completed: [...state.completed, currentMission.id],
        current: currentMission.id + 1,
      };
      setState(nextState);
      saveMissionState(nextState);

      const nextMission = MISSIONS.find(m => m.id === currentMission.id + 1);
      addToast('success', nextMission
        ? t('missions.completed', {
            mission: t(`missions.${currentMission.key}`, currentMission.key),
            next: t(`missions.${nextMission.key}`, nextMission.key),
            defaultValue: `Mission complete! Next: ${nextMission.key}`,
          })
        : t('missions.allComplete', 'All missions complete! Your intelligence system is calibrated.'));
    }
  }, [currentMission, getProgress, state, addToast, t]);

  // Listen for autophagy events
  useEffect(() => {
    const handler = () => {
      if (state.current === 5 && !state.completed.includes(5)) {
        const next = { ...state, completed: [...state.completed, 5], current: 6 };
        setState(next);
        saveMissionState(next);
      }
    };
    // @ts-expect-error -- Tauri event
    const unlisten = window.__TAURI__?.event?.listen?.('autophagy-cycle-complete', handler);
    return () => { unlisten?.then?.((fn: () => void) => fn()); };
  }, [state]);

  // Don't show if dismissed or all complete
  if (state.dismissed || !currentMission) return null;

  // Don't show if started more than 72 hours ago
  const hoursElapsed = (Date.now() - new Date(state.startedAt).getTime()) / 3600000;
  if (hoursElapsed > 72) return null;

  const progress = getProgress(currentMission.id);
  const pct = Math.min(100, Math.round((progress / currentMission.target) * 100));

  return (
    <div className="mb-3 px-4 py-3 bg-bg-secondary rounded-lg border border-border/50">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-text-muted uppercase tracking-wider font-medium">
            {t('missions.title', 'Getting Started')}
          </span>
          <span className="text-[10px] text-text-muted tabular-nums">
            {state.completed.length}/{MISSIONS.length}
          </span>
        </div>
        <button
          onClick={() => {
            const next = { ...state, dismissed: true };
            setState(next);
            saveMissionState(next);
          }}
          className="text-[10px] text-text-muted hover:text-text-secondary transition-colors"
          aria-label={t('missions.dismiss', 'Dismiss missions')}
        >
          {t('common.dismiss', 'Dismiss')}
        </button>
      </div>
      <div className="flex items-center gap-3">
        <div className="flex-1">
          <p className="text-xs text-text-primary font-medium">
            {t(`missions.${currentMission.key}`, currentMission.key)}
          </p>
          <div className="mt-1.5 h-1 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              className="h-full bg-blue-400 rounded-full transition-all duration-500"
              style={{ width: `${pct}%` }}
            />
          </div>
        </div>
        <span className="text-xs text-text-muted tabular-nums flex-shrink-0">
          {progress}/{currentMission.target}
        </span>
      </div>
    </div>
  );
});
