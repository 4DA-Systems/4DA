// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import { ViewErrorBoundary } from './ViewErrorBoundary';
import { ResultsView } from './ResultsView';

// Lazy-loaded views — each only loads when navigated to
const BriefingView = lazy(() => import('./BriefingView').then(m => ({ default: m.BriefingView })));
const SavedItemsView = lazy(() => import('./SavedItemsView').then(m => ({ default: m.SavedItemsView })));
const TechRadar = lazy(() => import('./TechRadar').then(m => ({ default: m.TechRadar })));
const DecisionMemory = lazy(() => import('./DecisionMemory').then(m => ({ default: m.DecisionMemory })));
const DelegationDashboard = lazy(() => import('./DelegationDashboard').then(m => ({ default: m.DelegationDashboard })));
const AgentMemoryPanel = lazy(() => import('./AgentMemoryPanel').then(m => ({ default: m.AgentMemoryPanel })));
const AutophagyInsights = lazy(() => import('./AutophagyInsights').then(m => ({ default: m.AutophagyInsights })));
const DecisionJournal = lazy(() => import('./DecisionJournal').then(m => ({ default: m.DecisionJournal })));
const AchievementsPanel = lazy(() => import('./AchievementsPanel').then(m => ({ default: m.AchievementsPanel })));
const SovereignDeveloperProfile = lazy(() => import('./SovereignDeveloperProfile').then(m => ({ default: m.SovereignDeveloperProfile })));
const ToolkitView = lazy(() => import('./toolkit/ToolkitView').then(m => ({ default: m.ToolkitView })));
const PlaybookView = lazy(() => import('./PlaybookView').then(m => ({ default: m.PlaybookView })));
const ChannelsView = lazy(() => import('./channels/ChannelsView').then(m => ({ default: m.ChannelsView })));
const CalibrationView = lazy(() => import('./CalibrationView').then(m => ({ default: m.CalibrationView })));
const SignalsPanel = lazy(() => import('./SignalsPanel').then(m => ({ default: m.SignalsPanel })));
const SignalChainsPanel = lazy(() => import('./SignalChains').then(m => ({ default: m.SignalChainsPanel })));
const KnowledgeGapsPanel = lazy(() => import('./KnowledgeGapsPanel').then(m => ({ default: m.KnowledgeGapsPanel })));

interface ViewRouterProps {
  newItemIds: Set<number>;
  focusedIndex: number;
}

export function ViewRouter({ newItemIds, focusedIndex }: ViewRouterProps) {
  const { t } = useTranslation();
  const { activeView, analysisComplete, relevanceResults } = useAppStore(
    useShallow(s => ({
      activeView: s.activeView,
      analysisComplete: s.appState.analysisComplete,
      relevanceResults: s.appState.relevanceResults,
    })),
  );

  return (
    <Suspense fallback={<div className="flex items-center justify-center py-20 text-text-secondary text-sm">{t('action.loading')}</div>}>
      {activeView === 'briefing' ? (
        <ViewErrorBoundary viewName="Briefing">
          <BriefingView />
        </ViewErrorBoundary>
      ) : activeView === 'channels' ? (
        <ViewErrorBoundary viewName="Channels">
          <ChannelsView />
        </ViewErrorBoundary>
      ) : activeView === 'profile' ? (
        <ViewErrorBoundary viewName="Profile">
          <SovereignDeveloperProfile />
        </ViewErrorBoundary>
      ) : activeView === 'insights' ? (
        <ViewErrorBoundary viewName="Insights">
          <section aria-label={t('nav.insights', { defaultValue: 'Insights' })} className="space-y-6">
            <AchievementsPanel />
            <TechRadar />
            <DecisionJournal />
            <DecisionMemory />
            <AutophagyInsights />
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              <DelegationDashboard />
              <AgentMemoryPanel />
            </div>
          </section>
        </ViewErrorBoundary>
      ) : activeView === 'saved' ? (
        <ViewErrorBoundary viewName="Saved">
          <SavedItemsView />
        </ViewErrorBoundary>
      ) : activeView === 'toolkit' ? (
        <ViewErrorBoundary viewName="Toolkit">
          <ToolkitView />
        </ViewErrorBoundary>
      ) : activeView === 'playbook' ? (
        <ViewErrorBoundary viewName="Playbook">
          <PlaybookView />
        </ViewErrorBoundary>
      ) : activeView === 'calibrate' ? (
        <ViewErrorBoundary viewName="Calibrate">
          <CalibrationView />
        </ViewErrorBoundary>
      ) : (
        <ViewErrorBoundary viewName="Results">
          {analysisComplete && (
            <Suspense fallback={null}>
              <SignalsPanel results={relevanceResults} />
              <SignalChainsPanel />
              <KnowledgeGapsPanel />
            </Suspense>
          )}
          <ResultsView
            newItemIds={newItemIds}
            focusedIndex={focusedIndex}
          />
        </ViewErrorBoundary>
      )}
    </Suspense>
  );
}
