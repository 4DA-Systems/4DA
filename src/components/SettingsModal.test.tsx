import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsModal } from './SettingsModal';
import { useAppStore } from '../store';

// Mock Tauri API (ScoreAutopsy, NaturalLanguageSearch, etc. use invoke)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock the Zustand store
vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    // Provide a minimal mock state that covers all the selectors SettingsModal uses
    const mockState: Record<string, unknown> = {
      settings: { llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null }, rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 }, usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 }, embedding_threshold: 0.25 },
      settingsForm: { provider: 'anthropic', apiKey: '', model: 'claude-3-5-haiku-20241022', baseUrl: '', openaiApiKey: '', embeddingThreshold: 0.25, rerankEnabled: false, maxItemsPerBatch: 10, minEmbeddingScore: 0.1, dailyTokenLimit: 100000, dailyCostLimitCents: 50 },
      setSettingsFormFull: vi.fn(),
      settingsStatus: '',
      setSettingsStatus: vi.fn(),
      saveSettings: vi.fn(),
      testConnection: vi.fn(),
      ollamaStatus: null,
      ollamaModels: [],
      checkOllamaStatus: vi.fn(),
      monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
      monitoringInterval: 30,
      notificationThreshold: 'high_and_above',
      setMonitoringInterval: vi.fn(),
      setNotificationThreshold: vi.fn(),
      toggleMonitoring: vi.fn(),
      updateMonitoringInterval: vi.fn(),
      testNotification: vi.fn(),
      scanDirectories: [],
      newScanDir: '',
      setNewScanDir: vi.fn(),
      isScanning: false,
      discoveredContext: null,
      runAutoDiscovery: vi.fn(),
      runFullScan: vi.fn(),
      addScanDirectory: vi.fn(),
      removeScanDirectory: vi.fn(),
      learnedAffinities: [],
      antiTopics: [],
      loadLearnedBehavior: vi.fn(),
      systemHealth: { anomalies: [], anomalyCount: 0, embeddingOperational: true, rateLimitStatus: null, accuracyMetrics: null },
      similarTopicQuery: '',
      setSimilarTopicQuery: vi.fn(),
      similarTopicResults: [],
      runAnomalyDetection: vi.fn(),
      resolveAnomaly: vi.fn(),
      findSimilarTopics: vi.fn(),
      saveWatcherState: vi.fn(),
      loadSystemHealth: vi.fn(),
      userContext: { role: null, tech_stack: [], domains: [], interests: [], exclusions: [], stats: { interest_count: 0, exclusion_count: 0 } },
      suggestedInterests: [],
      newInterest: '',
      setNewInterest: vi.fn(),
      newExclusion: '',
      setNewExclusion: vi.fn(),
      newTechStack: '',
      setNewTechStack: vi.fn(),
      newRole: '',
      setNewRole: vi.fn(),
      addInterest: vi.fn(),
      removeInterest: vi.fn(),
      addExclusion: vi.fn(),
      removeExclusion: vi.fn(),
      addTechStack: vi.fn(),
      removeTechStack: vi.fn(),
      updateRole: vi.fn(),
      loadSuggestedInterests: vi.fn(),
      loadSettings: vi.fn(),
      loadMonitoringStatus: vi.fn(),
      loadDiscoveredContext: vi.fn(),
      loadUserContext: vi.fn(),
      streetsTier: 'playbook',
      loadStreetsTier: vi.fn(),
      activateStreetsLicense: vi.fn(),
      loadLicense: vi.fn(),
      loadTrialStatus: vi.fn(),
      license: null,
      trialStatus: null,
    };
    return selector(mockState);
  }),
}));

// Mock child components to keep tests focused on SettingsModal itself
vi.mock('./LearnedBehaviorPanel', () => ({
  LearnedBehaviorPanel: () => <div data-testid="learned-behavior-panel" />,
}));

vi.mock('./SystemHealthPanel', () => ({
  SystemHealthPanel: () => <div data-testid="system-health-panel" />,
}));

vi.mock('./IndexedDocumentsPanel', () => ({
  IndexedDocumentsPanel: () => <div data-testid="indexed-documents-panel" />,
}));

vi.mock('./NaturalLanguageSearch', () => ({
  NaturalLanguageSearch: () => <div data-testid="natural-language-search" />,
}));

vi.mock('./SourceConfigPanel', () => ({
  SourceConfigPanel: () => <div data-testid="source-config-panel" />,
}));

vi.mock('./settings/AIProviderSection', () => ({
  AIProviderSection: () => <div data-testid="ai-provider-section" />,
}));

vi.mock('./settings/MonitoringSection', () => ({
  MonitoringSection: () => <div data-testid="monitoring-section" />,
}));

vi.mock('./settings/DigestSection', () => ({
  DigestSection: () => <div data-testid="digest-section" />,
}));

vi.mock('./settings/ContextDiscoverySection', () => ({
  ContextDiscoverySection: () => <div data-testid="context-discovery-section" />,
}));

vi.mock('./settings/PersonalizationSection', () => ({
  PersonalizationSection: () => <div data-testid="personalization-section" />,
}));

vi.mock('./settings/LocaleSection', () => ({
  LocaleSection: () => <div data-testid="locale-section" />,
}));

vi.mock('./ProValuePanel', () => ({
  ProValuePanel: () => <div data-testid="pro-value-panel" />,
}));

vi.mock('./DeveloperDna', () => ({
  DeveloperDnaPanel: () => <div data-testid="developer-dna-panel" />,
}));

vi.mock('./settings/AttentionDashboard', () => ({
  AttentionDashboard: () => <div data-testid="attention-dashboard" />,
}));

vi.mock('./settings/ProjectHealthRadar', () => ({
  ProjectHealthRadar: () => <div data-testid="project-health-radar" />,
}));

describe('SettingsModal', () => {
  it('does not render when not mounted (parent controls visibility)', () => {
    const show = false;
    const { container } = render(<div>{show && <SettingsModal onClose={vi.fn()} />}</div>);
    expect(container.querySelector('[role="dialog"]')).toBeNull();
  });

  it('renders the dialog when mounted', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('has role="dialog" and aria-modal="true"', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
  });

  it('has aria-labelledby pointing to the title', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-labelledby', 'settings-modal-title');
    expect(screen.getByText('settings.title')).toHaveAttribute('id', 'settings-modal-title');
  });

  it('calls onClose when close button is clicked', () => {
    const onClose = vi.fn();
    render(<SettingsModal onClose={onClose} />);

    const closeButton = screen.getByLabelText('Close settings');
    fireEvent.click(closeButton);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('renders the Settings title', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByText('settings.title')).toBeInTheDocument();
  });

  it('renders Save Settings and Test Connection in General tab', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    expect(screen.getByText('settings.saveSettings')).toBeInTheDocument();
    expect(screen.getByText('settings.testConnection')).toBeInTheDocument();
  });

  it('renders 6 tab buttons', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(6);
    expect(tabs.map(t => t.textContent)).toEqual(['settings.tabs.general', 'settings.tabs.sources', 'settings.tabs.profile', 'settings.tabs.projects', 'settings.tabs.advanced', 'settings.tabs.about']);
  });

  it('General tab is active by default', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    const generalTab = screen.getByRole('tab', { name: 'settings.tabs.general' });
    expect(generalTab).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('ai-provider-section')).toBeInTheDocument();
  });

  it('switches to Sources tab and shows SourceConfigPanel', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.sources' }));
    expect(screen.getByTestId('source-config-panel')).toBeInTheDocument();
    expect(screen.queryByTestId('ai-provider-section')).not.toBeInTheDocument();
  });

  it('switches to Profile tab and shows PersonalizationSection', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.profile' }));
    expect(screen.getByTestId('personalization-section')).toBeInTheDocument();
    expect(screen.getByTestId('developer-dna-panel')).toBeInTheDocument();
  });

  it('switches to Advanced tab and shows health panels', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.advanced' }));
    expect(screen.getByTestId('attention-dashboard')).toBeInTheDocument();
    expect(screen.getByTestId('system-health-panel')).toBeInTheDocument();
  });

  it('hides Save Settings when not on General tab', () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('tab', { name: 'settings.tabs.sources' }));
    expect(screen.queryByText('settings.saveSettings')).not.toBeInTheDocument();
  });

  // --- Error-path tests (Phase 3.2) ---

  it('shows error alert when settingsStatus contains "Error"', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      const mockState: Record<string, unknown> = {
        settings: { llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null }, rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 }, usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 }, embedding_threshold: 0.25 },
        settingsForm: { provider: 'anthropic', apiKey: '', model: 'claude-3-5-haiku-20241022', baseUrl: '', openaiApiKey: '', embeddingThreshold: 0.25, rerankEnabled: false, maxItemsPerBatch: 10, minEmbeddingScore: 0.1, dailyTokenLimit: 100000, dailyCostLimitCents: 50 },
        setSettingsFormFull: vi.fn(), settingsStatus: 'Error: Invalid API key', setSettingsStatus: vi.fn(),
        saveSettings: vi.fn(), testConnection: vi.fn(), ollamaStatus: null, ollamaModels: [],
        checkOllamaStatus: vi.fn(),
        monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
        monitoringInterval: 30, notificationThreshold: 'high_and_above',
        setMonitoringInterval: vi.fn(), setNotificationThreshold: vi.fn(), toggleMonitoring: vi.fn(),
        updateMonitoringInterval: vi.fn(), testNotification: vi.fn(),
        scanDirectories: [], newScanDir: '', setNewScanDir: vi.fn(), isScanning: false, discoveredContext: null,
        runAutoDiscovery: vi.fn(), runFullScan: vi.fn(), addScanDirectory: vi.fn(), removeScanDirectory: vi.fn(),
        learnedAffinities: [], antiTopics: [], loadLearnedBehavior: vi.fn(),
        systemHealth: { anomalies: [], anomalyCount: 0, embeddingOperational: true, rateLimitStatus: null, accuracyMetrics: null },
        similarTopicQuery: '', setSimilarTopicQuery: vi.fn(), similarTopicResults: [],
        runAnomalyDetection: vi.fn(), resolveAnomaly: vi.fn(), findSimilarTopics: vi.fn(),
        saveWatcherState: vi.fn(), loadSystemHealth: vi.fn(),
        userContext: { role: null, tech_stack: [], domains: [], interests: [], exclusions: [], stats: { interest_count: 0, exclusion_count: 0 } },
        suggestedInterests: [], newInterest: '', setNewInterest: vi.fn(), newExclusion: '', setNewExclusion: vi.fn(),
        newTechStack: '', setNewTechStack: vi.fn(), newRole: '', setNewRole: vi.fn(),
        addInterest: vi.fn(), removeInterest: vi.fn(), addExclusion: vi.fn(), removeExclusion: vi.fn(),
        addTechStack: vi.fn(), removeTechStack: vi.fn(), updateRole: vi.fn(),
        loadSuggestedInterests: vi.fn(), loadSettings: vi.fn(), loadMonitoringStatus: vi.fn(),
        loadDiscoveredContext: vi.fn(), loadUserContext: vi.fn(),
        streetsTier: 'playbook', loadStreetsTier: vi.fn(), activateStreetsLicense: vi.fn(),
        loadLicense: vi.fn(), loadTrialStatus: vi.fn(), license: null, trialStatus: null,
      };
      return selector(mockState);
    }) as any);
    render(<SettingsModal onClose={vi.fn()} />);
    // Use getByText to find status strip, then check its role attribute
    // (PanelErrorBoundary may also render role="alert" for unmocked child errors)
    const statusEl = screen.getByText('Error: Invalid API key');
    expect(statusEl.closest('[role="alert"]')).toBeInTheDocument();
    expect(statusEl).toBeVisible();
  });

  it('shows error alert when settingsStatus contains "failed"', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      const mockState: Record<string, unknown> = {
        settings: { llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null }, rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 }, usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 }, embedding_threshold: 0.25 },
        settingsForm: { provider: 'anthropic', apiKey: '', model: 'claude-3-5-haiku-20241022', baseUrl: '', openaiApiKey: '', embeddingThreshold: 0.25, rerankEnabled: false, maxItemsPerBatch: 10, minEmbeddingScore: 0.1, dailyTokenLimit: 100000, dailyCostLimitCents: 50 },
        setSettingsFormFull: vi.fn(), settingsStatus: 'Connection failed: timeout', setSettingsStatus: vi.fn(),
        saveSettings: vi.fn(), testConnection: vi.fn(), ollamaStatus: null, ollamaModels: [],
        checkOllamaStatus: vi.fn(),
        monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
        monitoringInterval: 30, notificationThreshold: 'high_and_above',
        setMonitoringInterval: vi.fn(), setNotificationThreshold: vi.fn(), toggleMonitoring: vi.fn(),
        updateMonitoringInterval: vi.fn(), testNotification: vi.fn(),
        scanDirectories: [], newScanDir: '', setNewScanDir: vi.fn(), isScanning: false, discoveredContext: null,
        runAutoDiscovery: vi.fn(), runFullScan: vi.fn(), addScanDirectory: vi.fn(), removeScanDirectory: vi.fn(),
        learnedAffinities: [], antiTopics: [], loadLearnedBehavior: vi.fn(),
        systemHealth: { anomalies: [], anomalyCount: 0, embeddingOperational: true, rateLimitStatus: null, accuracyMetrics: null },
        similarTopicQuery: '', setSimilarTopicQuery: vi.fn(), similarTopicResults: [],
        runAnomalyDetection: vi.fn(), resolveAnomaly: vi.fn(), findSimilarTopics: vi.fn(),
        saveWatcherState: vi.fn(), loadSystemHealth: vi.fn(),
        userContext: { role: null, tech_stack: [], domains: [], interests: [], exclusions: [], stats: { interest_count: 0, exclusion_count: 0 } },
        suggestedInterests: [], newInterest: '', setNewInterest: vi.fn(), newExclusion: '', setNewExclusion: vi.fn(),
        newTechStack: '', setNewTechStack: vi.fn(), newRole: '', setNewRole: vi.fn(),
        addInterest: vi.fn(), removeInterest: vi.fn(), addExclusion: vi.fn(), removeExclusion: vi.fn(),
        addTechStack: vi.fn(), removeTechStack: vi.fn(), updateRole: vi.fn(),
        loadSuggestedInterests: vi.fn(), loadSettings: vi.fn(), loadMonitoringStatus: vi.fn(),
        loadDiscoveredContext: vi.fn(), loadUserContext: vi.fn(),
        streetsTier: 'playbook', loadStreetsTier: vi.fn(), activateStreetsLicense: vi.fn(),
        loadLicense: vi.fn(), loadTrialStatus: vi.fn(), license: null, trialStatus: null,
      };
      return selector(mockState);
    }) as any);
    render(<SettingsModal onClose={vi.fn()} />);
    const statusEl = screen.getByText('Connection failed: timeout');
    expect(statusEl.closest('[role="alert"]')).toBeInTheDocument();
    expect(statusEl).toBeVisible();
  });

  it('shows success status with role="status" (not alert)', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      const mockState: Record<string, unknown> = {
        settings: { llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null }, rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 }, usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 }, embedding_threshold: 0.25 },
        settingsForm: { provider: 'anthropic', apiKey: '', model: 'claude-3-5-haiku-20241022', baseUrl: '', openaiApiKey: '', embeddingThreshold: 0.25, rerankEnabled: false, maxItemsPerBatch: 10, minEmbeddingScore: 0.1, dailyTokenLimit: 100000, dailyCostLimitCents: 50 },
        setSettingsFormFull: vi.fn(), settingsStatus: 'Settings saved!', setSettingsStatus: vi.fn(),
        saveSettings: vi.fn(), testConnection: vi.fn(), ollamaStatus: null, ollamaModels: [],
        checkOllamaStatus: vi.fn(),
        monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
        monitoringInterval: 30, notificationThreshold: 'high_and_above',
        setMonitoringInterval: vi.fn(), setNotificationThreshold: vi.fn(), toggleMonitoring: vi.fn(),
        updateMonitoringInterval: vi.fn(), testNotification: vi.fn(),
        scanDirectories: [], newScanDir: '', setNewScanDir: vi.fn(), isScanning: false, discoveredContext: null,
        runAutoDiscovery: vi.fn(), runFullScan: vi.fn(), addScanDirectory: vi.fn(), removeScanDirectory: vi.fn(),
        learnedAffinities: [], antiTopics: [], loadLearnedBehavior: vi.fn(),
        systemHealth: { anomalies: [], anomalyCount: 0, embeddingOperational: true, rateLimitStatus: null, accuracyMetrics: null },
        similarTopicQuery: '', setSimilarTopicQuery: vi.fn(), similarTopicResults: [],
        runAnomalyDetection: vi.fn(), resolveAnomaly: vi.fn(), findSimilarTopics: vi.fn(),
        saveWatcherState: vi.fn(), loadSystemHealth: vi.fn(),
        userContext: { role: null, tech_stack: [], domains: [], interests: [], exclusions: [], stats: { interest_count: 0, exclusion_count: 0 } },
        suggestedInterests: [], newInterest: '', setNewInterest: vi.fn(), newExclusion: '', setNewExclusion: vi.fn(),
        newTechStack: '', setNewTechStack: vi.fn(), newRole: '', setNewRole: vi.fn(),
        addInterest: vi.fn(), removeInterest: vi.fn(), addExclusion: vi.fn(), removeExclusion: vi.fn(),
        addTechStack: vi.fn(), removeTechStack: vi.fn(), updateRole: vi.fn(),
        loadSuggestedInterests: vi.fn(), loadSettings: vi.fn(), loadMonitoringStatus: vi.fn(),
        loadDiscoveredContext: vi.fn(), loadUserContext: vi.fn(),
        streetsTier: 'playbook', loadStreetsTier: vi.fn(), activateStreetsLicense: vi.fn(),
        loadLicense: vi.fn(), loadTrialStatus: vi.fn(), license: null, trialStatus: null,
      };
      return selector(mockState);
    }) as any);
    render(<SettingsModal onClose={vi.fn()} />);
    const statusEl = screen.getByText('Settings saved!');
    expect(statusEl.closest('[role="status"]')).toBeInTheDocument();
    // The settings status strip itself should not be an alert
    expect(statusEl.closest('[role="alert"]')).not.toBeInTheDocument();
  });

  it('shows error alert when settingsStatus contains "Connection failed:"', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      const mockState: Record<string, unknown> = {
        settings: { llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null }, rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 }, usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 }, embedding_threshold: 0.25 },
        settingsForm: { provider: 'anthropic', apiKey: '', model: 'claude-3-5-haiku-20241022', baseUrl: '', openaiApiKey: '', embeddingThreshold: 0.25, rerankEnabled: false, maxItemsPerBatch: 10, minEmbeddingScore: 0.1, dailyTokenLimit: 100000, dailyCostLimitCents: 50 },
        setSettingsFormFull: vi.fn(), settingsStatus: 'Connection failed: ECONNREFUSED', setSettingsStatus: vi.fn(),
        saveSettings: vi.fn(), testConnection: vi.fn(), ollamaStatus: null, ollamaModels: [],
        checkOllamaStatus: vi.fn(),
        monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
        monitoringInterval: 30, notificationThreshold: 'high_and_above',
        setMonitoringInterval: vi.fn(), setNotificationThreshold: vi.fn(), toggleMonitoring: vi.fn(),
        updateMonitoringInterval: vi.fn(), testNotification: vi.fn(),
        scanDirectories: [], newScanDir: '', setNewScanDir: vi.fn(), isScanning: false, discoveredContext: null,
        runAutoDiscovery: vi.fn(), runFullScan: vi.fn(), addScanDirectory: vi.fn(), removeScanDirectory: vi.fn(),
        learnedAffinities: [], antiTopics: [], loadLearnedBehavior: vi.fn(),
        systemHealth: { anomalies: [], anomalyCount: 0, embeddingOperational: true, rateLimitStatus: null, accuracyMetrics: null },
        similarTopicQuery: '', setSimilarTopicQuery: vi.fn(), similarTopicResults: [],
        runAnomalyDetection: vi.fn(), resolveAnomaly: vi.fn(), findSimilarTopics: vi.fn(),
        saveWatcherState: vi.fn(), loadSystemHealth: vi.fn(),
        userContext: { role: null, tech_stack: [], domains: [], interests: [], exclusions: [], stats: { interest_count: 0, exclusion_count: 0 } },
        suggestedInterests: [], newInterest: '', setNewInterest: vi.fn(), newExclusion: '', setNewExclusion: vi.fn(),
        newTechStack: '', setNewTechStack: vi.fn(), newRole: '', setNewRole: vi.fn(),
        addInterest: vi.fn(), removeInterest: vi.fn(), addExclusion: vi.fn(), removeExclusion: vi.fn(),
        addTechStack: vi.fn(), removeTechStack: vi.fn(), updateRole: vi.fn(),
        loadSuggestedInterests: vi.fn(), loadSettings: vi.fn(), loadMonitoringStatus: vi.fn(),
        loadDiscoveredContext: vi.fn(), loadUserContext: vi.fn(),
        streetsTier: 'playbook', loadStreetsTier: vi.fn(), activateStreetsLicense: vi.fn(),
        loadLicense: vi.fn(), loadTrialStatus: vi.fn(), license: null, trialStatus: null,
      };
      return selector(mockState);
    }) as any);
    render(<SettingsModal onClose={vi.fn()} />);
    const statusEl = screen.getByText('Connection failed: ECONNREFUSED');
    expect(statusEl.closest('[role="alert"]')).toBeInTheDocument();
    expect(statusEl).toBeVisible();
  });

  it('does not show settings status strip when settingsStatus is empty', () => {
    // Restore the default mock (previous tests override it with mockImplementation)
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    vi.mocked(useAppStore).mockImplementation(((selector: any) => {
      const mockState: Record<string, unknown> = {
        settings: { llm: { provider: 'anthropic', model: 'claude-3-5-haiku-20241022', has_api_key: false, base_url: null }, rerank: { enabled: false, max_items_per_batch: 10, min_embedding_score: 0.1, daily_token_limit: 100000, daily_cost_limit_cents: 50 }, usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 }, embedding_threshold: 0.25 },
        settingsForm: { provider: 'anthropic', apiKey: '', model: 'claude-3-5-haiku-20241022', baseUrl: '', openaiApiKey: '', embeddingThreshold: 0.25, rerankEnabled: false, maxItemsPerBatch: 10, minEmbeddingScore: 0.1, dailyTokenLimit: 100000, dailyCostLimitCents: 50 },
        setSettingsFormFull: vi.fn(), settingsStatus: '', setSettingsStatus: vi.fn(),
        saveSettings: vi.fn(), testConnection: vi.fn(), ollamaStatus: null, ollamaModels: [],
        checkOllamaStatus: vi.fn(),
        monitoring: { enabled: false, interval_minutes: 30, is_checking: false, last_check_ago: null, total_checks: 0 },
        monitoringInterval: 30, notificationThreshold: 'high_and_above',
        setMonitoringInterval: vi.fn(), setNotificationThreshold: vi.fn(), toggleMonitoring: vi.fn(),
        updateMonitoringInterval: vi.fn(), testNotification: vi.fn(),
        scanDirectories: [], newScanDir: '', setNewScanDir: vi.fn(), isScanning: false, discoveredContext: null,
        runAutoDiscovery: vi.fn(), runFullScan: vi.fn(), addScanDirectory: vi.fn(), removeScanDirectory: vi.fn(),
        learnedAffinities: [], antiTopics: [], loadLearnedBehavior: vi.fn(),
        systemHealth: { anomalies: [], anomalyCount: 0, embeddingOperational: true, rateLimitStatus: null, accuracyMetrics: null },
        similarTopicQuery: '', setSimilarTopicQuery: vi.fn(), similarTopicResults: [],
        runAnomalyDetection: vi.fn(), resolveAnomaly: vi.fn(), findSimilarTopics: vi.fn(),
        saveWatcherState: vi.fn(), loadSystemHealth: vi.fn(),
        userContext: { role: null, tech_stack: [], domains: [], interests: [], exclusions: [], stats: { interest_count: 0, exclusion_count: 0 } },
        suggestedInterests: [], newInterest: '', setNewInterest: vi.fn(), newExclusion: '', setNewExclusion: vi.fn(),
        newTechStack: '', setNewTechStack: vi.fn(), newRole: '', setNewRole: vi.fn(),
        addInterest: vi.fn(), removeInterest: vi.fn(), addExclusion: vi.fn(), removeExclusion: vi.fn(),
        addTechStack: vi.fn(), removeTechStack: vi.fn(), updateRole: vi.fn(),
        loadSuggestedInterests: vi.fn(), loadSettings: vi.fn(), loadMonitoringStatus: vi.fn(),
        loadDiscoveredContext: vi.fn(), loadUserContext: vi.fn(),
        streetsTier: 'playbook', loadStreetsTier: vi.fn(), activateStreetsLicense: vi.fn(),
        loadLicense: vi.fn(), loadTrialStatus: vi.fn(), license: null, trialStatus: null,
      };
      return selector(mockState);
    }) as any);
    const { container } = render(<SettingsModal onClose={vi.fn()} />);
    // The status strip has class "mx-6 mt-4" and renders inside the dialog
    // When settingsStatus is empty, the conditional {settingsStatus && ...} prevents rendering
    const statusStrip = container.querySelector('.mx-6.mt-4[role="alert"], .mx-6.mt-4[role="status"]');
    expect(statusStrip).toBeNull();
  });
});
