import { useState, useEffect, Component, ErrorInfo, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';
import { SplashScreen } from './components/SplashScreen';
import { Onboarding } from './components/Onboarding';
import { ResultItem } from './components/ResultItem';
import { SettingsModal } from './components/SettingsModal';
import { VoidEngine } from './components/void-engine/VoidEngine';
import { SignalsPanel } from './components/SignalsPanel';
import { NaturalLanguageSearch } from './components/NaturalLanguageSearch';
import {
  useSettings,
  useMonitoring,
  useAnalysis,
  useContextDiscovery,
  useFeedback,
  useSystemHealth,
  useUserContext,
} from './hooks';
import { getStageLabel } from './utils/score';

// Error Boundary to catch rendering errors
interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class ErrorBoundary extends Component<{ children: ReactNode }, ErrorBoundaryState> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('React Error Boundary caught:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={{
          backgroundColor: '#0A0A0A',
          color: '#fff',
          minHeight: '100vh',
          padding: '2rem',
          fontFamily: 'Inter, sans-serif',
        }}>
          <h1 style={{ color: '#EF4444' }}>Something went wrong</h1>
          <pre style={{
            backgroundColor: '#141414',
            padding: '1rem',
            borderRadius: '8px',
            overflow: 'auto',
            color: '#A0A0A0',
          }}>
            {this.state.error?.message}
            {'\n\n'}
            {this.state.error?.stack}
          </pre>
          <button
            onClick={() => window.location.reload()}
            style={{
              marginTop: '1rem',
              padding: '0.5rem 1rem',
              backgroundColor: '#2A2A2A',
              color: '#fff',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer',
            }}
          >
            Reload App
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

function App() {
  // Local UI state
  const [showSplash, setShowSplash] = useState(true);
  const [showSettings, setShowSettings] = useState(false);
  // Result filtering and sorting state
  const [sourceFilters, setSourceFilters] = useState<Set<string>>(new Set(['hackernews', 'arxiv', 'reddit', 'github', 'rss', 'youtube', 'twitter', 'producthunt']));
  const [sortBy, setSortBy] = useState<'score' | 'date'>('score');
  const [showOnlyRelevant, setShowOnlyRelevant] = useState(false);

  // All application state via hooks
  const {
    settings,
    settingsForm,
    setSettingsForm,
    settingsStatus,
    setSettingsStatus,
    showOnboarding,
    setShowOnboarding,
    loadSettings,
    saveSettings,
    testConnection,
    ollamaStatus,
    ollamaModels,
    checkOllamaStatus,
  } = useSettings();

  const {
    monitoring,
    monitoringInterval,
    setMonitoringInterval,
    toggleMonitoring,
    updateMonitoringInterval,
    testNotification,
  } = useMonitoring();

  const {
    state,
    expandedItem,
    setExpandedItem,
    isBrowserMode,
    enabledSources,
    loadContextFiles,
    clearContext,
    indexContext,
    startAnalysis,
    toggleSource,
  } = useAnalysis();

  const {
    scanDirectories,
    newScanDir,
    setNewScanDir,
    isScanning,
    discoveredContext,
    loadDiscoveredContext,
    runAutoDiscovery,
    runFullScan,
    addScanDirectory,
    removeScanDirectory,
  } = useContextDiscovery(setSettingsStatus);

  const {
    feedbackGiven,
    learnedAffinities,
    antiTopics,
    loadLearnedBehavior,
    recordInteraction,
  } = useFeedback(setSettingsStatus);

  const {
    systemHealth,
    similarTopicQuery,
    setSimilarTopicQuery,
    similarTopicResults,
    loadSystemHealth,
    runAnomalyDetection,
    resolveAnomaly,
    findSimilarTopics,
    saveWatcherState,
  } = useSystemHealth(setSettingsStatus);

  const {
    userContext,
    newInterest,
    setNewInterest,
    newExclusion,
    setNewExclusion,
    newTechStack,
    setNewTechStack,
    newRole,
    setNewRole,
    loadUserContext,
    addInterest,
    removeInterest,
    addExclusion,
    removeExclusion,
    addTechStack,
    removeTechStack,
    updateRole,
  } = useUserContext(setSettingsStatus);

  // Check Ollama status when provider changes to "ollama"
  useEffect(() => {
    if (settingsForm.provider === 'ollama') {
      checkOllamaStatus(settingsForm.baseUrl || undefined);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- only re-check when provider changes, not on every baseUrl keystroke
  }, [settingsForm.provider, checkOllamaStatus]);

  // AI Briefing state
  const [aiBriefing, setAiBriefing] = useState<{
    content: string | null;
    loading: boolean;
    error: string | null;
    model: string | null;
    lastGenerated: Date | null;
  }>({
    content: null,
    loading: false,
    error: null,
    model: null,
    lastGenerated: null,
  });
  const [showBriefing, setShowBriefing] = useState(false);

  // Generate AI briefing
  const generateBriefing = async () => {
    setAiBriefing(prev => ({ ...prev, loading: true, error: null }));
    try {
      const result = await invoke<{
        success: boolean;
        briefing: string | null;
        error?: string;
        model?: string;
        item_count?: number;
        latency_ms?: number;
      }>('generate_ai_briefing');

      if (result.success && result.briefing) {
        setAiBriefing({
          content: result.briefing,
          loading: false,
          error: null,
          model: result.model || null,
          lastGenerated: new Date(),
        });
        setShowBriefing(true);
      } else {
        setAiBriefing(prev => ({
          ...prev,
          loading: false,
          error: result.error || 'Failed to generate briefing',
        }));
      }
    } catch (error) {
      setAiBriefing(prev => ({
        ...prev,
        loading: false,
        error: `Error: ${error}`,
      }));
    }
  };

  // Toggle source filter
  const toggleSourceFilter = (source: string) => {
    setSourceFilters(prev => {
      const next = new Set(prev);
      if (next.has(source)) {
        if (next.size > 1) next.delete(source); // Keep at least one
      } else {
        next.add(source);
      }
      return next;
    });
  };

  // Filtered and sorted results
  const filteredResults = state.relevanceResults
    .filter(item => {
      // Source filter
      const source = item.source_type || 'hackernews';
      if (!sourceFilters.has(source)) return false;
      // Relevance filter
      if (showOnlyRelevant && !item.relevant) return false;
      return true;
    })
    .sort((a, b) => {
      if (sortBy === 'score') {
        return b.top_score - a.top_score;
      }
      // Sort by ID as proxy for date (higher ID = more recent)
      return b.id - a.id;
    });

  // Batch operations
  const dismissAllBelow = async (threshold: number) => {
    const itemsToDismiss = filteredResults.filter(
      item => item.top_score < threshold && !feedbackGiven[item.id],
    );
    for (const item of itemsToDismiss) {
      await recordInteraction(item.id, 'dismiss', item);
    }
    setSettingsStatus(`Dismissed ${itemsToDismiss.length} items below ${Math.round(threshold * 100)}%`);
    setTimeout(() => setSettingsStatus(''), 3000);
  };

  const saveAllAbove = async (threshold: number) => {
    const itemsToSave = filteredResults.filter(
      item => item.top_score >= threshold && !feedbackGiven[item.id],
    );
    for (const item of itemsToSave) {
      await recordInteraction(item.id, 'save', item);
    }
    setSettingsStatus(`Saved ${itemsToSave.length} items above ${Math.round(threshold * 100)}%`);
    setTimeout(() => setSettingsStatus(''), 3000);
  };

  // Auto-analyze on first load if monitoring enabled
  useEffect(() => {
    const autoAnalyzeTimer = setTimeout(async () => {
      try {
        const status = await invoke<{ enabled: boolean; total_checks: number }>('get_monitoring_status');
        if (status.enabled && status.total_checks === 0) {
          console.log('[4DA] Auto-triggering initial analysis (cache-first)...');
          await invoke('run_cached_analysis');
        }
      } catch (error) {
        console.log('Auto-analyze check failed:', error);
      }
    }, 3000);

    return () => clearTimeout(autoAnalyzeTimer);
  }, []);

  // Auto-briefing state
  const [autoBriefingEnabled, setAutoBriefingEnabled] = useState(true);
  const [lastBriefingCount, setLastBriefingCount] = useState(0);

  // Autonomous AI Briefing - triggers when analysis completes with new relevant items
  useEffect(() => {
    // Only trigger if:
    // 1. Auto-briefing is enabled
    // 2. Analysis just completed (analysisComplete is true)
    // 3. We have results
    // 4. Not already loading a briefing
    // 5. Results count changed (new analysis, not re-render)
    const totalCount = state.relevanceResults.length;
    const relevantCount = state.relevanceResults.filter(r => r.relevant).length;

    if (
      autoBriefingEnabled &&
      state.analysisComplete &&
      totalCount > 0 &&
      !aiBriefing.loading &&
      totalCount !== lastBriefingCount
    ) {
      console.log(`[4DA] Analysis complete (${relevantCount}/${totalCount} relevant), auto-generating AI briefing...`);
      setLastBriefingCount(totalCount);

      // Small delay to let UI settle
      const briefingTimer = setTimeout(() => {
        generateBriefing();
      }, 500);

      return () => clearTimeout(briefingTimer);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- trigger on analysis complete and count change, not on full results array
  }, [state.analysisComplete, state.relevanceResults.length, autoBriefingEnabled, aiBriefing.loading]);

  return (
    <>
      {/* Splash Screen */}
      {showSplash && (
        <SplashScreen onComplete={() => setShowSplash(false)} minimumDisplayTime={2500} />
      )}

      {/* Onboarding Flow (first run) */}
      {!showSplash && showOnboarding && (
        <Onboarding onComplete={() => {
          setShowOnboarding(false);
          loadSettings();
          loadUserContext();
          loadDiscoveredContext();
        }} />
      )}

      <div className={`min-h-screen bg-[#0A0A0A] text-white p-6 ${showSplash || showOnboarding ? 'hidden' : 'opacity-100 transition-opacity duration-300'}`}>
        {/* Header - Polished */}
        <header className="mb-8 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 flex items-center justify-center flex-shrink-0">
              <VoidEngine size={48} />
            </div>
            <div>
              <h1 className="text-2xl font-semibold tracking-tight text-white">4DA</h1>
              <p className="text-gray-500 text-sm">The internet searches for you</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            {monitoring?.enabled && (
              <div className="flex items-center gap-2 px-3 py-2 bg-green-500/10 border border-green-500/30 rounded-lg">
                <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                <span className="text-xs text-green-400 font-medium">Live</span>
              </div>
            )}
            <button
              onClick={() => setShowSettings(true)}
              className="px-4 py-2 text-sm bg-[#141414] text-gray-300 border border-[#2A2A2A] rounded-lg hover:bg-[#1F1F1F] hover:border-orange-500/30 transition-all"
            >
              ⚙️ Settings
            </button>
          </div>
        </header>

        {/* Browser Mode Warning */}
        {isBrowserMode && (
          <div className="mb-6 px-4 py-4 bg-red-900/20 border border-red-500/30 rounded-lg flex items-center gap-4">
            <div className="w-10 h-10 bg-red-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
              <span className="text-red-400">⚠️</span>
            </div>
            <div className="flex-1">
              <p className="text-sm font-medium text-red-300">Running in Browser Mode</p>
              <p className="text-xs text-gray-500 mt-1">Open 4DA through the desktop app for full functionality.</p>
            </div>
          </div>
        )}

        {/* Action Bar - Polished Card Style */}
        <div className="mb-6 bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
          {/* Main Action Row */}
          <div className="px-5 py-4 flex items-center gap-4">
            {/* Status */}
            <div className="flex items-center gap-3 flex-1 min-w-0">
              {state.loading ? (
                <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
                  <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                </div>
              ) : (
                <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center flex-shrink-0">
                  <span className="text-gray-500">📊</span>
                </div>
              )}
              <div className="min-w-0">
                <p className="text-sm text-white font-medium truncate">{state.loading ? 'Analyzing...' : 'Ready to analyze'}</p>
                <p className="text-xs text-gray-500 truncate">{state.status}</p>
              </div>
            </div>

            {/* Source Toggles */}
            <div className="flex items-center gap-2 px-3 py-1.5 bg-[#1F1F1F] rounded-lg">
              {[
                { id: 'hackernews', label: 'HN', icon: '🔶' },
                { id: 'arxiv', label: 'arXiv', icon: '📄' },
                { id: 'reddit', label: 'Reddit', icon: '🔴' },
              ].map(source => (
                <button
                  key={source.id}
                  onClick={() => toggleSource(source.id)}
                  disabled={state.loading}
                  className={`px-3 py-1.5 text-xs rounded-lg transition-all ${
                    enabledSources.includes(source.id)
                      ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
                      : 'bg-transparent text-gray-500 border border-transparent hover:text-gray-300'
                  } disabled:opacity-50`}
                >
                  {source.label}
                </button>
              ))}
            </div>

            {/* LLM Badge */}
            {settings?.rerank.enabled && settings?.llm.has_api_key && (
              <div className="px-3 py-1.5 bg-orange-500/10 text-orange-400 text-xs rounded-lg border border-orange-500/20">
                🤖 LLM Active
              </div>
            )}

            {/* Actions */}
            <div className="flex items-center gap-2">
              <button
                onClick={startAnalysis}
                disabled={state.loading}
                className="px-5 py-2.5 text-sm bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 transition-all disabled:opacity-50 disabled:cursor-not-allowed hover:scale-105 active:scale-95"
              >
                {state.loading ? 'Analyzing...' : '🔍 Analyze'}
              </button>
              <button
                onClick={generateBriefing}
                disabled={aiBriefing.loading || state.relevanceResults.length === 0}
                className="px-4 py-2.5 text-sm bg-[#1F1F1F] text-orange-400 border border-orange-500/30 font-medium rounded-lg hover:bg-orange-500/10 transition-all disabled:opacity-30 disabled:cursor-not-allowed flex items-center gap-2"
                title={state.relevanceResults.length === 0 ? 'Run analysis first' : 'Generate AI insights'}
              >
                {aiBriefing.loading ? (
                  <>
                    <div className="w-3 h-3 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
                    Thinking...
                  </>
                ) : (
                  <>✨ AI Brief</>
                )}
              </button>
              <button
                onClick={() => setAutoBriefingEnabled(!autoBriefingEnabled)}
                className={`w-10 h-10 rounded-lg flex items-center justify-center transition-all ${
                  autoBriefingEnabled
                    ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
                    : 'bg-[#1F1F1F] text-gray-500 border border-[#2A2A2A] hover:text-gray-300'
                }`}
                title={autoBriefingEnabled ? 'Auto-briefing ON' : 'Auto-briefing OFF'}
              >
                {autoBriefingEnabled ? '⚡' : '○'}
              </button>
            </div>
          </div>

          {/* Progress Bar */}
          {state.loading && state.progress > 0 && (
            <div className="px-5 pb-4">
              <div className="flex justify-between text-xs text-gray-500 mb-2">
                <span>{getStageLabel(state.progressStage)}</span>
                <span>{Math.round(state.progress * 100)}%</span>
              </div>
              <div className="w-full h-2 bg-[#1F1F1F] rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-300 ease-out rounded-full"
                  style={{ width: `${state.progress * 100}%` }}
                />
              </div>
            </div>
          )}

          {/* AI Briefing Error */}
          {aiBriefing.error && (
            <div className="mx-5 mb-4 p-3 bg-red-900/20 border border-red-500/30 rounded-lg text-red-300 text-sm flex items-center gap-2">
              <span>⚠️</span>
              {aiBriefing.error}
            </div>
          )}
        </div>

        {/* AI Briefing Panel - Polished */}
        {showBriefing && aiBriefing.content && (
          <div className="mb-6 bg-[#141414] rounded-lg border border-orange-500/30 overflow-hidden">
            <div className="px-5 py-4 border-b border-orange-500/20 flex items-center justify-between bg-orange-500/5">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                  <span className="text-orange-400">✨</span>
                </div>
                <div>
                  <h2 className="font-medium text-orange-400">AI Briefing</h2>
                  {aiBriefing.model && (
                    <span className="text-xs text-gray-500">via {aiBriefing.model}</span>
                  )}
                </div>
              </div>
              <button
                onClick={() => setShowBriefing(false)}
                className="w-8 h-8 rounded-lg bg-[#1F1F1F] text-gray-500 hover:text-white hover:bg-[#2A2A2A] flex items-center justify-center transition-all"
                title="Close briefing"
              >
                ×
              </button>
            </div>
            <div className="p-5 text-sm text-gray-300 leading-relaxed whitespace-pre-wrap">
              {aiBriefing.content.split('\n').map((line, i) => {
                // Style headers
                if (line.startsWith('## ')) {
                  return (
                    <h3 key={i} className="text-orange-400 font-medium mt-4 mb-2 first:mt-0">
                      {line.replace('## ', '')}
                    </h3>
                  );
                }
                // Style list items
                if (line.startsWith('- ') || line.startsWith('* ')) {
                  return (
                    <p key={i} className="ml-4 my-1">
                      <span className="text-orange-400 mr-2">•</span>
                      {line.replace(/^[-*] /, '')}
                    </p>
                  );
                }
                // Style numbered items
                if (/^\d+\. /.test(line)) {
                  return (
                    <p key={i} className="ml-4 my-1">
                      <span className="text-orange-400 mr-2">{line.match(/^\d+/)?.[0]}.</span>
                      {line.replace(/^\d+\. /, '')}
                    </p>
                  );
                }
                // Empty lines
                if (!line.trim()) {
                  return <div key={i} className="h-2" />;
                }
                // Regular text
                return <p key={i} className="my-1">{line}</p>;
              })}
            </div>
            {aiBriefing.lastGenerated && (
              <div className="px-5 py-3 border-t border-[#2A2A2A] text-xs text-gray-500">
                Generated {aiBriefing.lastGenerated.toLocaleTimeString()}
              </div>
            )}
          </div>
        )}

        {/* Actionable Signals */}
        {state.analysisComplete && (
          <SignalsPanel results={state.relevanceResults} />
        )}

        {/* Natural Language Search */}
        <div className="mb-6">
          <NaturalLanguageSearch defaultExpanded={false} />
        </div>

        {/* Main Content */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Context Files Panel - Polished */}
          <section className="bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
            <div className="px-5 py-4 border-b border-[#2A2A2A] flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center">
                  <span className="text-gray-500">📁</span>
                </div>
                <div>
                  <h2 className="font-medium text-white">Context</h2>
                  <p className="text-xs text-gray-500">{state.contextFiles.length} files indexed</p>
                </div>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={loadContextFiles}
                  className="w-8 h-8 flex items-center justify-center text-sm bg-[#1F1F1F] text-gray-400 rounded-lg hover:bg-[#2A2A2A] hover:text-white transition-all"
                  title="Reload files"
                >
                  ↻
                </button>
                {state.contextFiles.length > 0 && (
                  <>
                    <button
                      onClick={indexContext}
                      disabled={state.loading}
                      className="px-3 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/30 rounded-lg hover:bg-green-500/20 transition-all disabled:opacity-50"
                      title="Index files"
                    >
                      Index
                    </button>
                    <button
                      onClick={clearContext}
                      className="px-3 py-1.5 text-xs bg-red-500/10 text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/20 transition-all"
                      title="Clear"
                    >
                      Clear
                    </button>
                  </>
                )}
              </div>
            </div>
            <div className="p-4 max-h-[calc(100vh-320px)] overflow-y-auto">
              {state.contextFiles.length === 0 ? (
                <div className="text-center py-8 px-4">
                  <div className="w-12 h-12 mx-auto mb-3 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                    <span className="text-2xl">📂</span>
                  </div>
                  <p className="text-gray-400 text-sm mb-1">No context files yet</p>
                  <p className="text-xs text-gray-600">Add files to your context directory to enable personalized analysis.</p>
                </div>
              ) : (
                <ul className="space-y-2">
                  {state.contextFiles.map((file) => (
                    <li
                      key={file.path}
                      className="px-3 py-2 bg-[#1F1F1F] rounded-lg border border-[#2A2A2A] hover:border-orange-500/30 transition-all"
                    >
                      <div className="font-mono text-white text-sm truncate">
                        {file.path.split('/').pop()?.split('\\').pop()}
                      </div>
                      <div className="text-xs text-gray-500 mt-1">{file.lines} lines</div>
                    </li>
                  ))}
                </ul>
              )}

              {/* ACE Discovered Context */}
              {(discoveredContext.tech.length > 0 || discoveredContext.topics.length > 0) && (
                <div className="mt-4 pt-4 border-t border-[#2A2A2A]">
                  <div className="text-xs text-gray-500 mb-3 flex items-center gap-2">
                    <span>🔍 Auto-Discovered</span>
                    <span className="px-1.5 py-0.5 text-[10px] bg-orange-500/20 text-orange-400 rounded">ACE</span>
                  </div>
                  {discoveredContext.tech.length > 0 && (
                    <div className="mb-3">
                      <div className="flex flex-wrap gap-1.5">
                        {discoveredContext.tech.slice(0, 6).map((tech) => (
                          <span
                            key={tech.name}
                            className="px-2 py-1 text-[11px] bg-green-500/10 text-green-400 rounded-lg border border-green-500/20"
                            title={`${tech.category} - ${Math.round(tech.confidence * 100)}%`}
                          >
                            {tech.name}
                          </span>
                        ))}
                        {discoveredContext.tech.length > 6 && (
                          <span className="text-[11px] text-gray-500 self-center">+{discoveredContext.tech.length - 6}</span>
                        )}
                      </div>
                    </div>
                  )}
                  {discoveredContext.topics.length > 0 && (
                    <div className="flex flex-wrap gap-1.5">
                      {discoveredContext.topics.slice(0, 4).map((topic) => (
                        <span
                          key={topic}
                          className="px-2 py-1 text-[11px] bg-orange-500/10 text-orange-400 rounded-lg border border-orange-500/20"
                        >
                          {topic}
                        </span>
                      ))}
                      {discoveredContext.topics.length > 4 && (
                        <span className="text-[11px] text-gray-500 self-center">+{discoveredContext.topics.length - 4}</span>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>
          </section>

          {/* Relevance Results Panel - Polished */}
          <section className="lg:col-span-2 bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
            <div className="px-5 py-4 border-b border-[#2A2A2A]">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center">
                    <span className="text-gray-500">📊</span>
                  </div>
                  <div>
                    <h2 className="font-medium text-white">Results</h2>
                    <p className="text-xs text-gray-500">
                      {state.analysisComplete
                        ? `${filteredResults.length} items • ${filteredResults.filter((r) => r.relevant).length} relevant`
                        : 'Click Analyze to find relevant content'}
                    </p>
                  </div>
                </div>
                {state.analysisComplete && (
                  <div className="flex items-center gap-2">
                    {filteredResults.filter((r) => r.top_score >= 0.6).length > 0 && (
                      <span className="text-xs px-2 py-1 bg-orange-500/10 text-orange-400 rounded-lg">
                        ⭐ {filteredResults.filter((r) => r.top_score >= 0.6).length} top picks
                      </span>
                    )}
                    <span className="text-xs px-2 py-1 bg-green-500/10 text-green-400 rounded-lg">
                      {filteredResults.filter((r) => r.relevant).length} relevant
                    </span>
                  </div>
                )}
              </div>

              {/* Filter Bar - Polished */}
              {state.analysisComplete && (
                <div className="flex flex-wrap items-center gap-3 pt-3 border-t border-[#2A2A2A]">
                  {/* Source Filters - dynamic based on results */}
                  <div className="flex items-center gap-2 bg-[#1F1F1F] px-3 py-1.5 rounded-lg flex-wrap">
                    <span className="text-xs text-gray-500">Sources:</span>
                    {(() => {
                      const sourceLabels: Record<string, string> = {
                        hackernews: 'HN', arxiv: 'arXiv', reddit: 'Reddit',
                        github: 'GitHub', rss: 'RSS', youtube: 'YouTube',
                        twitter: 'Twitter', producthunt: 'PH',
                      };
                      const presentSources = [...new Set(state.relevanceResults.map(r => r.source_type || 'hackernews'))];
                      return presentSources
                        .sort((a, b) => (sourceLabels[a] || a).localeCompare(sourceLabels[b] || b))
                        .map(id => (
                        <button
                          key={id}
                          onClick={() => toggleSourceFilter(id)}
                          className={`px-2 py-1 text-xs rounded-lg transition-all ${
                            sourceFilters.has(id)
                              ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
                              : 'text-gray-500 hover:text-gray-300'
                          }`}
                        >
                          {sourceLabels[id] || id}
                        </button>
                      ));
                    })()}
                  </div>

                  {/* Sort */}
                  <div className="flex items-center gap-2 bg-[#1F1F1F] px-3 py-1.5 rounded-lg">
                    <span className="text-xs text-gray-500">Sort:</span>
                    <button
                      onClick={() => setSortBy('score')}
                      className={`px-2 py-1 text-xs rounded-lg transition-all ${
                        sortBy === 'score'
                          ? 'bg-white/10 text-white'
                          : 'text-gray-500 hover:text-gray-300'
                      }`}
                    >
                      Score
                    </button>
                    <button
                      onClick={() => setSortBy('date')}
                      className={`px-2 py-1 text-xs rounded-lg transition-all ${
                        sortBy === 'date'
                          ? 'bg-white/10 text-white'
                          : 'text-gray-500 hover:text-gray-300'
                      }`}
                    >
                      Recent
                    </button>
                  </div>

                  {/* Relevance Toggle */}
                  <button
                    onClick={() => setShowOnlyRelevant(!showOnlyRelevant)}
                    className={`px-3 py-1.5 text-xs rounded-lg transition-all ${
                      showOnlyRelevant
                        ? 'bg-green-500/20 text-green-400 border border-green-500/30'
                        : 'bg-[#1F1F1F] text-gray-500 hover:text-gray-300'
                    }`}
                  >
                    {showOnlyRelevant ? '✓ Relevant only' : 'Show all'}
                  </button>

                  {/* Spacer */}
                  <div className="flex-1" />

                  {/* Batch Operations */}
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => dismissAllBelow(0.3)}
                      className="px-3 py-1.5 text-xs bg-[#1F1F1F] text-gray-500 rounded-lg hover:bg-red-500/10 hover:text-red-400 transition-all"
                      title="Dismiss all items below 30% relevance"
                    >
                      ✕ &lt;30%
                    </button>
                    <button
                      onClick={() => saveAllAbove(0.6)}
                      className="px-3 py-1.5 text-xs bg-[#1F1F1F] text-gray-500 rounded-lg hover:bg-green-500/10 hover:text-green-400 transition-all"
                      title="Save all items above 60% relevance"
                    >
                      ✓ &gt;60%
                    </button>
                  </div>
                </div>
              )}
            </div>
            <div className="p-4 max-h-[calc(100vh-380px)] overflow-y-auto">
              {!state.analysisComplete ? (
                <div className="text-center py-16">
                  {state.loading ? (
                    <>
                      <div className="w-16 h-16 mx-auto mb-4 bg-orange-500/20 rounded-full flex items-center justify-center">
                        <div className="w-8 h-8 border-3 border-orange-500 border-t-transparent rounded-full animate-spin" />
                      </div>
                      <p className="text-lg text-white mb-2">Analyzing...</p>
                      <p className="text-sm text-gray-500">{state.progressMessage}</p>
                    </>
                  ) : (
                    <>
                      <div className="w-16 h-16 mx-auto mb-4 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                        <span className="text-3xl">🔍</span>
                      </div>
                      <p className="text-lg text-white mb-2">Ready to search</p>
                      <p className="text-sm text-gray-500">
                        Click <span className="text-orange-400">Analyze</span> to find relevant content
                      </p>
                    </>
                  )}
                </div>
              ) : filteredResults.length === 0 ? (
                <div className="text-center py-16">
                  <div className="w-16 h-16 mx-auto mb-4 bg-[#1F1F1F] rounded-full flex items-center justify-center">
                    <span className="text-3xl">🤷</span>
                  </div>
                  <p className="text-lg text-white mb-2">No results match</p>
                  <p className="text-sm text-gray-500">
                    Try enabling more sources or showing all items
                  </p>
                </div>
              ) : (
                <ul className="space-y-3">
                  {filteredResults.map((item) => (
                    <ResultItem
                      key={item.id}
                      item={item}
                      isExpanded={expandedItem === item.id}
                      onToggleExpand={() => setExpandedItem(expandedItem === item.id ? null : item.id)}
                      feedbackGiven={feedbackGiven}
                      onRecordInteraction={recordInteraction}
                    />
                  ))}
                </ul>
              )}
            </div>
          </section>
        </div>

        {/* Footer - Polished */}
        <footer className="mt-8 text-center">
          <p className="text-xs text-gray-600">The internet searches for you</p>
        </footer>

        {/* Settings Modal */}
        {showSettings && (
          <SettingsModal
            onClose={() => setShowSettings(false)}
            settings={settings}
            settingsForm={settingsForm}
            setSettingsForm={setSettingsForm}
            settingsStatus={settingsStatus}
            setSettingsStatus={setSettingsStatus}
            saveSettings={saveSettings}
            testConnection={testConnection}
            ollamaStatus={ollamaStatus}
            ollamaModels={ollamaModels}
            monitoring={monitoring}
            monitoringInterval={monitoringInterval}
            setMonitoringInterval={setMonitoringInterval}
            toggleMonitoring={toggleMonitoring}
            updateMonitoringInterval={updateMonitoringInterval}
            testNotification={testNotification}
            scanDirectories={scanDirectories}
            newScanDir={newScanDir}
            setNewScanDir={setNewScanDir}
            isScanning={isScanning}
            discoveredContext={discoveredContext}
            runAutoDiscovery={runAutoDiscovery}
            runFullScan={runFullScan}
            addScanDirectory={addScanDirectory}
            removeScanDirectory={removeScanDirectory}
            learnedAffinities={learnedAffinities}
            antiTopics={antiTopics}
            loadLearnedBehavior={loadLearnedBehavior}
            systemHealth={systemHealth}
            similarTopicQuery={similarTopicQuery}
            setSimilarTopicQuery={setSimilarTopicQuery}
            similarTopicResults={similarTopicResults}
            runAnomalyDetection={runAnomalyDetection}
            resolveAnomaly={resolveAnomaly}
            findSimilarTopics={findSimilarTopics}
            saveWatcherState={saveWatcherState}
            loadSystemHealth={loadSystemHealth}
            userContext={userContext}
            newInterest={newInterest}
            setNewInterest={setNewInterest}
            newExclusion={newExclusion}
            setNewExclusion={setNewExclusion}
            newTechStack={newTechStack}
            setNewTechStack={setNewTechStack}
            newRole={newRole}
            setNewRole={setNewRole}
            addInterest={addInterest}
            removeInterest={removeInterest}
            addExclusion={addExclusion}
            removeExclusion={removeExclusion}
            addTechStack={addTechStack}
            removeTechStack={removeTechStack}
            updateRole={updateRole}
          />
        )}
      </div>
    </>
  );
}

// Wrap App with ErrorBoundary
function AppWithErrorBoundary() {
  return (
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  );
}

export default AppWithErrorBoundary;
