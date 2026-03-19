import { memo } from 'react';
import { PanelErrorBoundary } from '../PanelErrorBoundary';
import { ProValuePanel } from '../ProValuePanel';
import { AttentionDashboard } from './AttentionDashboard';
import { NaturalLanguageQueryPanel } from '../NaturalLanguageQuery';
import { ProjectHealthRadar } from './ProjectHealthRadar';
import { SystemHealthPanel } from '../SystemHealthPanel';

interface SettingsAdvancedTabProps {
  systemHealth: any;
  similarTopicQuery: string;
  setSimilarTopicQuery: (q: string) => void;
  similarTopicResults: any[];
  runAnomalyDetection: () => void;
  resolveAnomaly: (anomalyId: number) => void;
  findSimilarTopics: () => void;
  saveWatcherState: () => void;
  loadSystemHealth: () => void;
}

export const SettingsAdvancedTab = memo(function SettingsAdvancedTab({
  systemHealth,
  similarTopicQuery,
  setSimilarTopicQuery,
  similarTopicResults,
  runAnomalyDetection,
  resolveAnomaly,
  findSimilarTopics,
  saveWatcherState,
  loadSystemHealth,
}: SettingsAdvancedTabProps) {
  return (
    <div id="tabpanel-advanced" role="tabpanel">
      <div className="space-y-6">
        <PanelErrorBoundary name="Pro Value">
          <ProValuePanel />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Attention Dashboard">
          <AttentionDashboard />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Query Panel">
          <NaturalLanguageQueryPanel />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Project Health">
          <ProjectHealthRadar />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="System Health">
          <SystemHealthPanel
            health={systemHealth}
            similarTopicQuery={similarTopicQuery}
            onSimilarTopicQueryChange={setSimilarTopicQuery}
            similarTopicResults={similarTopicResults}
            onRunAnomalyDetection={runAnomalyDetection}
            onResolveAnomaly={resolveAnomaly}
            onFindSimilarTopics={findSimilarTopics}
            onSaveWatcherState={saveWatcherState}
            onRefresh={loadSystemHealth}
          />
        </PanelErrorBoundary>
      </div>
    </div>
  );
});
