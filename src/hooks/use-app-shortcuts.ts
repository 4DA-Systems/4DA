// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

import { useKeyboardShortcuts } from './use-keyboard-shortcuts';
import { useAppStore } from '../store';
import type { AppState, ActiveView } from '../store/types';
import type { SourceRelevance } from '../types/analysis';

interface AppShortcutDeps {
  showSettings: boolean;
  setShowSettings: (v: boolean) => void;
  showKeyboardHelp: boolean;
  setShowKeyboardHelp: (v: boolean) => void;
  expandedItem: number | null;
  setExpandedItem: (v: number | null) => void;
  state: AppState;
  startAnalysis: () => Promise<void>;
  showOnlyRelevant: boolean;
  setShowOnlyRelevant: (v: boolean) => void;
  activeView: ActiveView;
  setActiveView: (v: ActiveView) => void;
  filteredResults: SourceRelevance[];
  addToast: (type: 'info' | 'success' | 'error' | 'warning', message: string) => void;
}

/**
 * Wires global keyboard shortcuts to App-level state and actions.
 * Extracted from App.tsx to reduce its line count.
 */
export function useAppShortcuts(deps: AppShortcutDeps) {
  const { t } = useTranslation();
  const [analysisPulse, setAnalysisPulse] = useState(false);

  const {
    showSettings, setShowSettings,
    showKeyboardHelp, setShowKeyboardHelp,
    expandedItem, setExpandedItem,
    state, startAnalysis,
    showOnlyRelevant, setShowOnlyRelevant,
    activeView, setActiveView,
    filteredResults,
    addToast,
  } = deps;

  const { focusedIndex } = useKeyboardShortcuts({
    onAnalyze: () => { void startAnalysis(); },
    onToggleFilters: () => setShowOnlyRelevant(!showOnlyRelevant),
    onToggleBriefing: () => setActiveView(activeView === 'briefing' ? 'results' : 'briefing'),
    onOpenSettings: () => setShowSettings(true),
    onEscape: () => {
      if (showKeyboardHelp) { setShowKeyboardHelp(false); return; }
      if (showSettings) { setShowSettings(false); return; }
      if (expandedItem !== null) { setExpandedItem(null); return; }
    },
    onHelp: () => setShowKeyboardHelp(true),
    analyzeDisabled: state.loading,
    briefingAvailable: true,
    filtersAvailable: state.analysisComplete,
    resultCount: filteredResults.length,
    onFocusResult: (index: number) => {
      const el = document.getElementById(`result-item-${filteredResults[index]?.id}`);
      el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    },
    onToggleExpandResult: (index: number) => {
      const item = filteredResults[index];
      if (item) setExpandedItem(expandedItem === item.id ? null : item.id);
    },
    onOpenResult: (index: number) => {
      const item = filteredResults[index];
      if (item?.url) window.open(item.url, '_blank', 'noopener,noreferrer');
    },
    onAnalyzeTriggered: () => {
      addToast('info', t('analysis.keyboardTriggered'));
      setAnalysisPulse(true);
      setTimeout(() => setAnalysisPulse(false), 500);
    },
    onSaveFocused: () => {
      const item = filteredResults[focusedIndex];
      if (item) void useAppStore.getState().recordInteraction(item.id, 'save', item);
    },
    onDismissFocused: () => {
      const item = filteredResults[focusedIndex];
      if (item) void useAppStore.getState().recordInteraction(item.id, 'dismiss', item);
    },
    onFocusSearch: () => {
      const el = document.querySelector<HTMLInputElement>('[data-search-input]');
      el?.focus();
    },
  });

  const handleAnalyze = useCallback(() => {
    void startAnalysis();
  }, [startAnalysis]);

  return { focusedIndex, analysisPulse, handleAnalyze };
}
