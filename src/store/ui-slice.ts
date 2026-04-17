import type { StateCreator } from 'zustand';
import type { AppStore, UiSlice, ViewTier } from './types';

const STORAGE_KEY = '4da-progressive-disclosure';

interface PersistedDisclosure {
  analysisCycleCount: number;
  firstAnalysisDate: string | null;
  showAllViews: boolean;
}

function loadPersistedDisclosure(): PersistedDisclosure {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<PersistedDisclosure>;
      return {
        analysisCycleCount: parsed.analysisCycleCount ?? 0,
        firstAnalysisDate: parsed.firstAnalysisDate ?? null,
        showAllViews: parsed.showAllViews ?? false,
      };
    }
  } catch { /* ignore corrupt data */ }
  return { analysisCycleCount: 0, firstAnalysisDate: null, showAllViews: false };
}

function persistDisclosure(data: PersistedDisclosure): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
  } catch { /* ignore quota errors */ }
}

type ViewId = 'briefing' | 'results' | 'saved' | 'toolkit' | 'playbook' | 'profile' | 'calibrate' | 'console' | 'preemption' | 'blindspots';

// CANONICAL SOURCE: What setActiveView will ACCEPT per tier.
// MUST stay in sync with TIER_VIEWS in src/components/ViewTabBar.tsx.
// Exported for the consistency test at src/components/__tests__/tier-views-consistency.test.ts.
// If they diverge, tabs show up visually but clicking them silently fails.
// 2026-04-16 — Intelligence Reconciliation: removed 'insights' (Momentum deleted)
// and 'chapters' (CategoryChapterView merged into Results).
export const UI_SLICE_TIER_VIEWS: Record<ViewTier, ViewId[]> = {
  core: ['briefing', 'results', 'playbook'],
  explorer: ['briefing', 'preemption', 'blindspots', 'results', 'playbook'],
  invested: ['briefing', 'preemption', 'blindspots', 'results', 'playbook', 'saved', 'profile', 'console'],
  power: ['briefing', 'preemption', 'blindspots', 'results', 'playbook', 'saved', 'profile', 'console', 'toolkit', 'calibrate'],
};

const TIER_ORDER: ViewTier[] = ['core', 'explorer', 'invested', 'power'];

const TIER_UPGRADE_MESSAGES: Partial<Record<ViewTier, string>> = {
  explorer: 'New views unlocked: Preemption & Blind Spots',
  invested: 'New views unlocked: Saved & Profile',
  power: 'All views unlocked: Toolkit & System',
};

const persisted = loadPersistedDisclosure();

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set, get) => ({
  showSettings: false,
  showSplash: true,
  activeView: 'briefing',
  isFirstRun: false,
  firstRunDismissed: false,
  embeddingMode: null,
  embeddingStatus: undefined,
  viewTier: 'core',
  showAllViews: persisted.showAllViews,
  analysisCycleCount: persisted.analysisCycleCount,
  firstAnalysisDate: persisted.firstAnalysisDate,

  setShowSettings: (show) => set({ showSettings: show }),
  setShowSplash: (show) => set({ showSplash: show }),
  setActiveView: (view) => {
    const { viewTier, showAllViews } = get();
    if (!showAllViews) {
      const allowed = UI_SLICE_TIER_VIEWS[viewTier];
      if (!allowed.includes(view as ViewId)) return;
    }
    set({ activeView: view });
  },
  setIsFirstRun: (v) => set({ isFirstRun: v }),
  setFirstRunDismissed: (v) => set({ firstRunDismissed: v }),
  setEmbeddingMode: (mode) => set({ embeddingMode: mode }),
  setEmbeddingStatus: (status) => set({ embeddingStatus: status }),

  incrementAnalysisCycle: () => {
    const state = get();
    const newCount = state.analysisCycleCount + 1;
    const newFirstDate = state.firstAnalysisDate ?? new Date().toISOString();

    set({
      analysisCycleCount: newCount,
      firstAnalysisDate: newFirstDate,
    });

    persistDisclosure({
      analysisCycleCount: newCount,
      firstAnalysisDate: newFirstDate,
      showAllViews: state.showAllViews,
    });

    // Recompute tier after incrementing
    setTimeout(() => get().computeViewTier(), 0);
  },

  setShowAllViews: (show) => {
    set({ showAllViews: show });
    const state = get();
    persistDisclosure({
      analysisCycleCount: state.analysisCycleCount,
      firstAnalysisDate: state.firstAnalysisDate,
      showAllViews: show,
    });
  },

  computeViewTier: () => {
    const state = get();
    const { analysisCycleCount, firstAnalysisDate, feedbackGiven, decisions } = state;
    const previousTier = state.viewTier;

    // Count saved items from feedbackGiven
    const savedCount = Object.values(feedbackGiven).filter(a => a === 'save').length;
    const decisionsCount = decisions.length;

    // Determine new tier
    let newTier: ViewTier = 'core';

    if (analysisCycleCount >= 3) {
      newTier = 'explorer';
    }

    if (newTier === 'explorer' && (savedCount >= 5 || decisionsCount >= 2)) {
      newTier = 'invested';
    }

    if (newTier === 'invested' && firstAnalysisDate) {
      const firstDate = new Date(firstAnalysisDate);
      const now = new Date();
      const daysDiff = (now.getTime() - firstDate.getTime()) / (1000 * 60 * 60 * 24);
      if (daysDiff >= 14) {
        newTier = 'power';
      }
    }

    set({ viewTier: newTier });

    // Toast on upgrade
    if (TIER_ORDER.indexOf(newTier) > TIER_ORDER.indexOf(previousTier)) {
      const message = TIER_UPGRADE_MESSAGES[newTier];
      if (message) {
        state.addToast('success', message);
      }
    }
  },
});
