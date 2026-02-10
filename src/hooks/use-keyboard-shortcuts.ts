import { useEffect, useRef } from 'react';

interface KeyboardShortcutActions {
  onAnalyze: () => void;
  onToggleFilters: () => void;
  onToggleBriefing: () => void;
  onOpenSettings: () => void;
  onEscape: () => void;
  /** Whether the analyze action is currently disabled (e.g., loading) */
  analyzeDisabled?: boolean;
  /** Whether the briefing toggle is available (has content) */
  briefingAvailable?: boolean;
  /** Whether the filter toggle is available (analysis complete) */
  filtersAvailable?: boolean;
}

export function useKeyboardShortcuts(actions: KeyboardShortcutActions): void {
  // Use refs to avoid re-registering the listener when callbacks change
  const actionsRef = useRef(actions);
  useEffect(() => {
    actionsRef.current = actions;
  });

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

      const current = actionsRef.current;

      if (e.key === 'Escape') {
        current.onEscape();
        return;
      }

      if (e.key === 'r' && !e.ctrlKey && !e.metaKey && !current.analyzeDisabled) {
        current.onAnalyze();
        return;
      }

      if (e.key === 'b' && current.briefingAvailable) {
        current.onToggleBriefing();
        return;
      }

      if (e.key === ',') {
        current.onOpenSettings();
        return;
      }

      if (e.key === 'f' && current.filtersAvailable) {
        current.onToggleFilters();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);
}
