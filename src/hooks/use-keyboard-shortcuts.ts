// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useRef, useState, useCallback } from 'react';


interface KeyboardShortcutActions {
  onAnalyze: () => void;
  onToggleFilters: () => void;
  onToggleBriefing: () => void;
  onOpenSettings: () => void;
  onEscape: () => void;
  onHelp?: () => void;
  /** Whether the analyze action is currently disabled (e.g., loading) */
  analyzeDisabled?: boolean;
  /** Whether the briefing toggle is available (has content) */
  briefingAvailable?: boolean;
  /** Whether the filter toggle is available (analysis complete) */
  filtersAvailable?: boolean;
  /** Total result count for j/k navigation bounds */
  resultCount?: number;
  /** Callback when focused item changes (for expanding) */
  onFocusResult?: (index: number) => void;
  /** Callback to toggle expand on focused item */
  onToggleExpandResult?: (index: number) => void;
  /** Callback to open focused item URL */
  onOpenResult?: (index: number) => void;
  /** Callback fired when analysis is triggered via keyboard shortcut */
  onAnalyzeTriggered?: () => void;
  /** Save the currently focused item */
  onSaveFocused?: () => void;
  /** Dismiss the currently focused item */
  onDismissFocused?: () => void;
  /** Focus the search input */
  onFocusSearch?: () => void;
}

export function useKeyboardShortcuts(actions: KeyboardShortcutActions) {
  const [focusedIndex, setFocusedIndex] = useState(-1);

  // Use refs to avoid re-registering the listener when callbacks change
  const actionsRef = useRef(actions);
  useEffect(() => {
    actionsRef.current = actions;
  });

  // Reset focus when result count changes
  const prevCount = useRef(actions.resultCount);
  useEffect(() => {
    if (actions.resultCount !== prevCount.current) {
      setFocusedIndex(-1);
      prevCount.current = actions.resultCount;
    }
  }, [actions.resultCount]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

      const current = actionsRef.current;
      const maxIndex = (current.resultCount ?? 0) - 1;

      if (e.key === 'Escape') {
        if (focusedIndex >= 0) {
          setFocusedIndex(-1);
          return;
        }
        current.onEscape();
        return;
      }

      // j/k/ArrowDown/ArrowUp: navigate results
      if ((e.key === 'j' || e.key === 'ArrowDown') && maxIndex >= 0) {
        e.preventDefault();
        setFocusedIndex(prev => {
          const next = Math.min(prev + 1, maxIndex);
          current.onFocusResult?.(next);
          return next;
        });
        return;
      }

      if ((e.key === 'k' || e.key === 'ArrowUp') && maxIndex >= 0) {
        e.preventDefault();
        setFocusedIndex(prev => {
          const next = Math.max(prev - 1, 0);
          current.onFocusResult?.(next);
          return next;
        });
        return;
      }

      // Enter/Space: toggle expand on focused item
      if ((e.key === 'Enter' || e.key === ' ') && focusedIndex >= 0) {
        e.preventDefault();
        current.onToggleExpandResult?.(focusedIndex);
        return;
      }

      // o: open focused item URL
      if (e.key === 'o' && focusedIndex >= 0) {
        e.preventDefault();
        current.onOpenResult?.(focusedIndex);
        return;
      }

      if (e.key === 'r' && !e.ctrlKey && !e.metaKey && !current.analyzeDisabled) {
        current.onAnalyze();
        current.onAnalyzeTriggered?.();
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

      if (e.key === '?' && current.onHelp) {
        current.onHelp();
        return;
      }

      if (e.key === 's' && current.onSaveFocused) {
        e.preventDefault();
        current.onSaveFocused();
        return;
      }
      if (e.key === 'd' && current.onDismissFocused) {
        e.preventDefault();
        current.onDismissFocused();
        return;
      }
      if (e.key === '/' && current.onFocusSearch) {
        e.preventDefault();
        current.onFocusSearch();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [focusedIndex]);

  const clearFocus = useCallback(() => setFocusedIndex(-1), []);

  return { focusedIndex, clearFocus };
}
