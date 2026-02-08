import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import sunLogo from '../assets/sun-logo.jpg';

interface OnboardingProps {
  onComplete: () => void;
}

type Step = 'welcome' | 'api-keys' | 'context' | 'interests' | 'first-scan' | 'complete';

interface ApiKeyState {
  anthropic: string;
  openai: string;
  xApiKey: string;
  provider: 'anthropic' | 'openai' | 'ollama';
}

interface OllamaStatus {
  running: boolean;
  version: string | null;
  models: string[];
  base_url: string;
  error?: string;
}

export function Onboarding({ onComplete }: OnboardingProps) {
  const [step, setStep] = useState<Step>('welcome');
  const [apiKeys, setApiKeys] = useState<ApiKeyState>({
    anthropic: '',
    openai: '',
    xApiKey: '',
    provider: 'anthropic',
  });
  const [role, setRole] = useState('');
  const [interests, setInterests] = useState<string[]>([]);
  const [newInterest, setNewInterest] = useState('');
  const [isDiscovering, setIsDiscovering] = useState(false);
  const [discoveryResult, setDiscoveryResult] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus | null>(null);
  const [isCheckingOllama, setIsCheckingOllama] = useState(false);
  const [selectedOllamaModel, setSelectedOllamaModel] = useState<string>('');
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [isAnimating, setIsAnimating] = useState(true);

  // First scan state
  const [isScanning, setIsScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState<{
    phase: 'fetching' | 'scoring' | 'done' | 'error';
    message: string;
    results?: Array<{ title: string; score: number; source: string }>;
    total?: number;
    relevant?: number;
  } | null>(null);

  const steps: Step[] = ['welcome', 'api-keys', 'context', 'interests', 'first-scan', 'complete'];
  const currentIndex = steps.indexOf(step);

  // Trigger entrance animation
  useEffect(() => {
    setIsAnimating(true);
    const timer = setTimeout(() => setIsAnimating(false), 300);
    return () => clearTimeout(timer);
  }, [step]);

  // Auto-check Ollama when provider is selected
  useEffect(() => {
    if (apiKeys.provider === 'ollama') {
      checkOllamaStatus();
    }
  }, [apiKeys.provider]);

  const checkOllamaStatus = async () => {
    setIsCheckingOllama(true);
    setError(null);
    try {
      const status = await invoke<OllamaStatus>('check_ollama_status', { baseUrl: null });
      setOllamaStatus(status);
      if (status.running && status.models.length > 0) {
        setSelectedOllamaModel(status.models[0]);
      }
    } catch {
      setOllamaStatus({ running: false, version: null, models: [], base_url: 'http://localhost:11434' });
    }
    setIsCheckingOllama(false);
  };

  const nextStep = () => {
    const next = steps[currentIndex + 1];
    if (next) setStep(next);
  };

  const prevStep = () => {
    const prev = steps[currentIndex - 1];
    if (prev) setStep(prev);
  };

  // Test API connection before saving
  const testApiConnection = async () => {
    try {
      setIsTesting(true);
      setError(null);
      setTestResult(null);

      const provider = apiKeys.provider;
      const apiKey = provider === 'anthropic' ? apiKeys.anthropic :
                     provider === 'openai' ? apiKeys.openai : '';
      const model = provider === 'anthropic' ? 'claude-3-haiku-20240307' :
                    provider === 'openai' ? 'gpt-4o-mini' :
                    (selectedOllamaModel || ollamaStatus?.models[0] || 'llama3.2');
      const baseUrl = provider === 'ollama' ? (ollamaStatus?.base_url || 'http://localhost:11434') : null;
      const openaiApiKey = provider !== 'openai' ? apiKeys.openai : null;

      // First save the settings
      await invoke('set_llm_provider', { provider, apiKey, model, baseUrl, openaiApiKey });

      // Then test the connection
      const result = await invoke<{ success: boolean; message: string }>('test_llm_connection');
      setTestResult(result);

      if (!result.success) {
        // Parse common errors into helpful messages
        let hint = '';
        if (result.message.includes('401') || result.message.includes('invalid')) {
          hint = 'Check that your API key is correct and has not expired.';
        } else if (result.message.includes('429') || result.message.includes('rate')) {
          hint = "You've hit a rate limit. Wait a moment and try again.";
        } else if (result.message.includes('network') || result.message.includes('connect')) {
          hint = 'Check your internet connection.';
        }
        if (hint) {
          setError(hint);
        }
      }
    } catch (e) {
      setTestResult({ success: false, message: String(e) });
      setError('Connection test failed. Check your settings and try again.');
    } finally {
      setIsTesting(false);
    }
  };

  const saveApiKeys = async () => {
    try {
      setError(null);
      // Save LLM provider settings using the correct command
      const provider = apiKeys.provider;
      const apiKey = provider === 'anthropic' ? apiKeys.anthropic :
                     provider === 'openai' ? apiKeys.openai : '';
      // Use selected Ollama model or fallback to first available
      const model = provider === 'anthropic' ? 'claude-3-haiku-20240307' :
                    provider === 'openai' ? 'gpt-4o-mini' :
                    (selectedOllamaModel || ollamaStatus?.models[0] || 'llama3.2');
      const baseUrl = provider === 'ollama' ? (ollamaStatus?.base_url || 'http://localhost:11434') : null;
      // Pass OpenAI key separately for embeddings (when using Anthropic/Ollama for LLM)
      const openaiApiKey = provider !== 'openai' ? apiKeys.openai : null;

      await invoke('set_llm_provider', {
        provider,
        apiKey,
        model,
        baseUrl,
        openaiApiKey,
      });
      // Save X API key if provided (validates against X API)
      if (apiKeys.xApiKey.trim()) {
        try {
          const result = await invoke('set_x_api_key', { key: apiKeys.xApiKey.trim() }) as { validated?: boolean; warning?: string };
          if (result?.warning) {
            console.warn('X API key warning:', result.warning);
          }
        } catch (xErr) {
          // Show X API error but don't block onboarding - it's optional
          setError(`X API key issue: ${xErr}\n\nYou can fix this later in Settings. Continuing...`);
          setTimeout(() => {
            setError(null);
            nextStep();
          }, 4000);
          return;
        }
      }
      nextStep();
    } catch (e) {
      setError(`Failed to save: ${e}`);
    }
  };

  // Skip API setup and use defaults (can configure later)
  const skipApiSetup = async () => {
    try {
      // Set Ollama as default if they skip (free, no API key needed)
      await invoke('set_llm_provider', {
        provider: 'ollama',
        apiKey: '',
        model: 'llama3.2',
        baseUrl: 'http://localhost:11434',
      });
      nextStep();
    } catch {
      // Even if this fails, let them continue
      nextStep();
    }
  };

  const runAutoDiscovery = async () => {
    try {
      setIsDiscovering(true);
      setError(null);
      const result = await invoke<{ directories_found: number; message: string }>('ace_auto_discover');
      setDiscoveryResult(`Found ${result.directories_found} project directories!`);
      setIsDiscovering(false);
    } catch (e) {
      setError(`Discovery failed: ${e}`);
      setIsDiscovering(false);
    }
  };

  const saveInterests = async () => {
    try {
      setError(null);
      // Save role
      if (role) {
        await invoke('set_user_role', { role });
      }
      // Save interests
      for (const interest of interests) {
        await invoke('add_interest', { topic: interest });
      }
      // Don't mark complete yet - wait for first scan
      nextStep();
    } catch (e) {
      setError(`Failed to save: ${e}`);
    }
  };

  const addInterest = () => {
    if (newInterest.trim() && !interests.includes(newInterest.trim())) {
      setInterests([...interests, newInterest.trim()]);
      setNewInterest('');
    }
  };

  // Run the DEEP first scan - comprehensive intelligence gathering for new users
  const runFirstScan = async () => {
    try {
      setIsScanning(true);
      setScanProgress({
        phase: 'fetching',
        message: 'Deep scanning HN (5 categories), arXiv (16 fields), Reddit (40+ subs)...',
      });

      // Run DEEP initial scan - fetches 300-500+ items from multiple endpoints
      await invoke('run_deep_initial_scan');

      setScanProgress({ phase: 'scoring', message: 'Analyzing hundreds of items for relevance...' });

      // Get results using compute_relevance
      const results = await invoke<Array<{
        id: number;
        title: string;
        url: string | null;
        top_score: number;
        source_type: string;
        relevant: boolean;
      }>>('compute_relevance');

      // Filter and sort results
      const relevant = results.filter((r) => r.relevant || r.top_score >= 0.3);
      const topPicks = results.filter((r) => r.top_score >= 0.6);
      const topResults = relevant
        .sort((a, b) => b.top_score - a.top_score)
        .slice(0, 8) // Show more top results since we have more data
        .map((r) => ({
          title: r.title,
          score: Math.round(r.top_score * 100),
          source: r.source_type === 'hackernews' ? 'HN' :
                  r.source_type === 'arxiv' ? 'arXiv' :
                  r.source_type === 'reddit' ? 'Reddit' :
                  r.source_type === 'github' ? 'GitHub' : r.source_type,
        }));

      setScanProgress({
        phase: 'done',
        message: `Found ${relevant.length} relevant items, ${topPicks.length} top picks!`,
        results: topResults,
        total: results.length,
        relevant: relevant.length,
      });

    } catch (e) {
      setScanProgress({
        phase: 'error',
        message: `Scan failed: ${e}. You can try again from the main app.`,
      });
    } finally {
      setIsScanning(false);
    }
  };

  // Auto-run scan when reaching first-scan step
  useEffect(() => {
    if (step === 'first-scan' && !scanProgress && !isScanning) {
      runFirstScan();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- only trigger scan when step changes to first-scan
  }, [step]);

  const removeInterest = (interest: string) => {
    setInterests(interests.filter(i => i !== interest));
  };

  const suggestedInterests = [
    'Machine Learning', 'Rust', 'TypeScript', 'Web Development',
    'DevOps', 'Security', 'Startups', 'Open Source', 'AI/LLM',
    'Mobile Development', 'Cloud Infrastructure', 'Data Engineering',
  ];

  // Step labels for progress
  const stepLabels: Record<Step, string> = {
    welcome: 'Welcome',
    'api-keys': 'AI Provider',
    context: 'Context',
    interests: 'Interests',
    'first-scan': 'First Scan',
    complete: 'Ready!',
  };

  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-[#0A0A0A] p-8">
      {/* Progress indicator - improved */}
      <div className="absolute top-8 flex items-center gap-1">
        {steps.map((s, i) => (
          <div key={s} className="flex items-center">
            <div
              className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-medium transition-all duration-300 ${
                i < currentIndex
                  ? 'bg-orange-500 text-white'
                  : i === currentIndex
                  ? 'bg-orange-500/20 text-orange-400 ring-2 ring-orange-500'
                  : 'bg-[#1F1F1F] text-gray-600'
              }`}
            >
              {i < currentIndex ? '✓' : i + 1}
            </div>
            {i < steps.length - 1 && (
              <div
                className={`w-8 h-0.5 transition-colors duration-300 ${
                  i < currentIndex ? 'bg-orange-500' : 'bg-[#1F1F1F]'
                }`}
              />
            )}
          </div>
        ))}
      </div>

      {/* Step label */}
      <div className="absolute top-[70px] text-xs text-gray-500">
        {stepLabels[step]}
      </div>

      {/* Error display - improved */}
      {error && (
        <div className="absolute top-24 max-w-md bg-red-900/30 border border-red-500/30 text-red-200 px-4 py-3 rounded-lg animate-in fade-in slide-in-from-top-2 duration-200">
          <div className="flex items-start gap-2">
            <span className="text-red-400 flex-shrink-0">⚠</span>
            <div>
              <div className="font-medium text-red-300 text-sm">Something went wrong</div>
              <div className="text-xs text-red-200/80 mt-1">{error}</div>
            </div>
            <button
              onClick={() => setError(null)}
              className="text-red-400 hover:text-red-300 ml-2"
            >
              ×
            </button>
          </div>
        </div>
      )}

      {/* Step content */}
      <div className="max-w-2xl w-full">
        {step === 'welcome' && (
          <div className={`text-center transition-all duration-500 ${isAnimating ? 'opacity-0 scale-95' : 'opacity-100 scale-100'}`}>
            <div className="w-32 h-32 mx-auto mb-6 rounded-full overflow-hidden shadow-2xl ring-4 ring-orange-500/20">
              <img src={sunLogo} alt="4DA" className="w-full h-full object-cover" />
            </div>
            <h1 className="text-4xl font-semibold text-white mb-3">Welcome to 4DA</h1>
            <p className="text-xl text-orange-400 mb-2 font-medium">The internet searches for you.</p>
            <p className="text-gray-500 mb-8 max-w-md mx-auto">
              4DA learns what you care about and surfaces relevant content from across the internet - before you know you need it.
            </p>
            <div className="space-y-3 text-left bg-[#141414] p-5 rounded-lg mb-8 max-w-md mx-auto">
              <ul className="text-gray-400 space-y-3">
                <li className="flex items-start gap-3">
                  <span className="flex-shrink-0 w-8 h-8 bg-green-500/20 rounded-lg flex items-center justify-center">
                    <span className="text-green-400">🔒</span>
                  </span>
                  <div>
                    <strong className="text-white block">100% Private</strong>
                    <span className="text-sm">All processing happens locally</span>
                  </div>
                </li>
                <li className="flex items-start gap-3">
                  <span className="flex-shrink-0 w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                    <span className="text-orange-400">⚡</span>
                  </span>
                  <div>
                    <strong className="text-white block">Autonomous</strong>
                    <span className="text-sm">Self-discovering, learns from you</span>
                  </div>
                </li>
                <li className="flex items-start gap-3">
                  <span className="flex-shrink-0 w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center">
                    <span className="text-blue-400">🔑</span>
                  </span>
                  <div>
                    <strong className="text-white block">BYOK</strong>
                    <span className="text-sm">Your API keys, you control costs</span>
                  </div>
                </li>
              </ul>
            </div>
            <button
              onClick={nextStep}
              className="px-10 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-medium hover:scale-105 active:scale-95"
            >
              Get Started →
            </button>
            <p className="text-xs text-gray-600 mt-4">Takes about 2 minutes to set up</p>
          </div>
        )}

        {step === 'api-keys' && (
          <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
            <h2 className="text-3xl font-semibold text-white mb-2 text-center">API Keys</h2>
            <p className="text-gray-400 mb-6 text-center">
              4DA needs API keys to analyze content and understand relevance.
            </p>

            <div className="space-y-4 mb-6">
              {/* OpenAI Key - REQUIRED for embeddings */}
              <div className="bg-[#141414] p-5 rounded-lg">
                <div className="flex items-center gap-2 mb-3">
                  <span className="px-2 py-0.5 bg-red-500/20 text-red-400 text-xs rounded font-medium">Required</span>
                  <h3 className="text-white font-medium">OpenAI API Key</h3>
                </div>
                <p className="text-sm text-gray-400 mb-3">
                  Used for semantic understanding (embeddings). This is required for 4DA to work.
                </p>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-xs text-gray-500">API Key</label>
                  <a
                    href="https://platform.openai.com/api-keys"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-xs text-orange-500 hover:underline"
                  >
                    Get your API key →
                  </a>
                </div>
                <input
                  type="password"
                  value={apiKeys.openai}
                  onChange={(e) => {
                    setApiKeys({ ...apiKeys, openai: e.target.value });
                    setTestResult(null);
                    setError(null);
                  }}
                  placeholder="sk-proj-..."
                  className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
                />
                <p className="text-xs text-gray-500 mt-2">
                  ~$0.02/1M tokens for embeddings. Typical usage costs pennies/month.
                </p>
              </div>

              {/* LLM Provider - for analysis/reasoning */}
              <div className="bg-[#141414] p-5 rounded-lg">
                <div className="flex items-center gap-2 mb-3">
                  <span className="px-2 py-0.5 bg-blue-500/20 text-blue-400 text-xs rounded font-medium">Analysis</span>
                  <h3 className="text-white font-medium">LLM Provider</h3>
                </div>
                <p className="text-sm text-gray-400 mb-3">
                  Choose which AI to use for analyzing and explaining relevance.
                </p>

                {/* Provider selection */}
                <div className="grid grid-cols-3 gap-2 mb-3">
                  {(['openai', 'anthropic', 'ollama'] as const).map((p) => (
                    <button
                      key={p}
                      onClick={() => {
                        setApiKeys({ ...apiKeys, provider: p });
                        setTestResult(null);
                        if (p === 'ollama') checkOllamaStatus();
                      }}
                      className={`p-3 rounded-lg text-center transition-all ${
                        apiKeys.provider === p
                          ? 'bg-orange-500/20 border-2 border-orange-500'
                          : 'bg-[#1F1F1F] border-2 border-transparent hover:border-[#2A2A2A]'
                      }`}
                    >
                      <div className="text-sm font-medium text-white">
                        {p === 'openai' ? 'OpenAI' : p === 'anthropic' ? 'Anthropic' : 'Ollama'}
                      </div>
                      <div className="text-xs text-gray-500 mt-1">
                        {p === 'openai' ? 'GPT-4o' : p === 'anthropic' ? 'Claude' : 'Local'}
                      </div>
                    </button>
                  ))}
                </div>

                {/* Provider-specific config */}
                {apiKeys.provider === 'anthropic' && (
                  <div className="mt-3">
                    <div className="flex items-center justify-between mb-2">
                      <label className="text-xs text-gray-500">Anthropic API Key</label>
                      <a
                        href="https://console.anthropic.com/settings/keys"
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-xs text-orange-500 hover:underline"
                      >
                        Get key →
                      </a>
                    </div>
                    <input
                      type="password"
                      value={apiKeys.anthropic}
                      onChange={(e) => {
                        setApiKeys({ ...apiKeys, anthropic: e.target.value });
                        setTestResult(null);
                      }}
                      placeholder="sk-ant-api03-..."
                      className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
                    />
                  </div>
                )}

                {apiKeys.provider === 'ollama' && (
                  <div className="mt-3 p-3 bg-[#1F1F1F] rounded-lg">
                    {isCheckingOllama ? (
                      <div className="flex items-center gap-2">
                        <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                        <span className="text-gray-400 text-sm">Checking Ollama...</span>
                      </div>
                    ) : ollamaStatus?.running ? (
                      <div className="space-y-2">
                        <div className="flex items-center gap-2">
                          <span className="text-green-500">✓</span>
                          <span className="text-green-300 text-sm">Ollama running (v{ollamaStatus.version})</span>
                        </div>
                        {ollamaStatus.models.length > 0 && (
                          <select
                            value={selectedOllamaModel}
                            onChange={(e) => setSelectedOllamaModel(e.target.value)}
                            className="w-full px-3 py-2 bg-[#141414] border border-[#2A2A2A] rounded text-white text-sm"
                          >
                            {ollamaStatus.models.map((model) => (
                              <option key={model} value={model}>{model}</option>
                            ))}
                          </select>
                        )}
                      </div>
                    ) : (
                      <div className="text-yellow-400 text-sm">
                        Ollama not detected.{' '}
                        <a href="https://ollama.ai" target="_blank" rel="noopener noreferrer" className="text-orange-500 hover:underline">
                          Install Ollama
                        </a>
                        {' '}or{' '}
                        <button onClick={checkOllamaStatus} className="text-orange-500 hover:underline">retry</button>
                      </div>
                    )}
                  </div>
                )}

                {apiKeys.provider === 'openai' && (
                  <p className="text-xs text-gray-500 mt-2">
                    Using same OpenAI key for both embeddings and analysis.
                  </p>
                )}
              </div>

              {/* X/Twitter API Key - Optional */}
              <div className="bg-[#141414] p-5 rounded-lg">
                <div className="flex items-center gap-2 mb-3">
                  <span className="px-2 py-0.5 bg-gray-500/20 text-gray-400 text-xs rounded font-medium">Optional</span>
                  <h3 className="text-white font-medium">X / Twitter API Key</h3>
                </div>
                <p className="text-sm text-gray-400 mb-3">
                  Enables monitoring tweets from tech influencers. Requires a free X Developer account.
                </p>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-xs text-gray-500">Bearer Token</label>
                  <a
                    href="https://developer.x.com/en/portal/dashboard"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-xs text-orange-500 hover:underline"
                  >
                    Get X Developer access →
                  </a>
                </div>
                <input
                  type="password"
                  value={apiKeys.xApiKey}
                  onChange={(e) => setApiKeys({ ...apiKeys, xApiKey: e.target.value })}
                  placeholder="Bearer token..."
                  className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
                />
                <p className="text-xs text-gray-500 mt-2">
                  Skip this if you don't have an X developer account. You can add it later in Settings.
                </p>
              </div>

              {/* Test connection button */}
              <button
                onClick={testApiConnection}
                disabled={isTesting || (!apiKeys.openai && apiKeys.provider !== 'ollama')}
                className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] text-gray-300 rounded-lg hover:bg-[#2A2A2A] transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
              >
                {isTesting ? (
                  <>
                    <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                    Testing connection...
                  </>
                ) : (
                  <>
                    <span>🔌</span>
                    Test Connection
                  </>
                )}
              </button>

              {/* Test result */}
              {testResult && (
                <div className={`p-3 rounded-lg text-sm ${
                  testResult.success
                    ? 'bg-green-900/30 text-green-300 border border-green-500/30'
                    : 'bg-red-900/30 text-red-300 border border-red-500/30'
                }`}>
                  {testResult.success ? (
                    <div className="flex items-center gap-2">
                      <span className="text-green-500">✓</span>
                      Connection successful! Ready to use.
                    </div>
                  ) : (
                    <div>
                      <div className="font-medium">Connection failed</div>
                      <div className="text-xs mt-1 opacity-80">{testResult.message}</div>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Navigation */}
            <div className="flex justify-between items-center">
              <button
                onClick={prevStep}
                className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
              >
                ← Back
              </button>
              <div className="flex items-center gap-3">
                <button
                  onClick={skipApiSetup}
                  className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
                >
                  Skip for now
                </button>
                <button
                  onClick={saveApiKeys}
                  disabled={!apiKeys.openai && apiKeys.provider !== 'ollama'}
                  className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Continue
                </button>
              </div>
            </div>
          </div>
        )}

        {step === 'context' && (
          <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
            <h2 className="text-3xl font-semibold text-white mb-2 text-center">Discover Your Context</h2>
            <p className="text-gray-400 mb-6 text-center">
              4DA learns what matters to you by scanning your projects. 100% local, 100% private.
            </p>

            <div className="bg-[#141414] p-6 rounded-lg mb-4">
              {/* Discovery state visualization */}
              <div className="flex items-center gap-4 mb-6">
                <div className={`w-16 h-16 rounded-full flex items-center justify-center ${
                  discoveryResult ? 'bg-green-500/20' : isDiscovering ? 'bg-orange-500/20' : 'bg-[#1F1F1F]'
                }`}>
                  {discoveryResult ? (
                    <span className="text-3xl">✓</span>
                  ) : isDiscovering ? (
                    <div className="w-8 h-8 border-3 border-orange-500 border-t-transparent rounded-full animate-spin" />
                  ) : (
                    <span className="text-3xl">📁</span>
                  )}
                </div>
                <div className="flex-1">
                  <h3 className="text-white font-medium">
                    {discoveryResult ? 'Discovery Complete!' : isDiscovering ? 'Scanning your projects...' : 'Auto-Discovery'}
                  </h3>
                  <p className="text-sm text-gray-400 mt-1">
                    {discoveryResult || (isDiscovering
                      ? 'Looking for code, notes, documents...'
                      : 'Scans ~/Projects, ~/Code, ~/Documents and similar locations'
                    )}
                  </p>
                </div>
              </div>

              {/* Discovery action or result */}
              {!discoveryResult && !isDiscovering && (
                <button
                  onClick={runAutoDiscovery}
                  className="w-full py-4 bg-orange-500/20 border-2 border-dashed border-orange-500/50 text-orange-300 rounded-lg hover:bg-orange-500/30 hover:border-orange-500 transition-all font-medium"
                >
                  <span className="text-lg">🔍</span> Scan My Computer
                </button>
              )}

              {isDiscovering && (
                <div className="w-full h-2 bg-[#1F1F1F] rounded-full overflow-hidden">
                  <div className="h-full bg-orange-500 rounded-full animate-pulse" style={{ width: '60%' }} />
                </div>
              )}

              {discoveryResult && (
                <div className="bg-green-900/20 border border-green-500/30 text-green-300 p-4 rounded-lg">
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    {discoveryResult}
                  </div>
                  <p className="text-xs text-green-400/70 mt-2">
                    4DA will continuously learn from your activity in these directories.
                  </p>
                </div>
              )}

              <p className="text-xs text-gray-500 mt-4 text-center">
                {discoveryResult
                  ? 'You can manage directories anytime in Settings'
                  : 'Or skip this and add directories manually later'
                }
              </p>
            </div>

            {/* FAQ Section */}
            <div className="bg-[#141414] rounded-lg p-4 mb-6">
              <details className="group">
                <summary className="flex items-center justify-between cursor-pointer text-sm text-gray-400 hover:text-gray-300 transition-colors">
                  <span className="flex items-center gap-2">
                    <span className="text-orange-400">?</span>
                    Common questions about context scanning
                  </span>
                  <span className="text-xs group-open:rotate-180 transition-transform">▼</span>
                </summary>
                <div className="mt-4 space-y-4 text-sm">
                  <div className="bg-[#1F1F1F] rounded-lg p-3">
                    <h4 className="text-white font-medium mb-1">What files are being scanned?</h4>
                    <p className="text-gray-400 text-xs">
                      4DA looks for project markers (package.json, Cargo.toml, README files, etc.),
                      code files, and documents. It reads file names and contents to understand your work context.
                    </p>
                  </div>

                  <div className="bg-[#1F1F1F] rounded-lg p-3">
                    <h4 className="text-white font-medium mb-1">Is my data sent anywhere?</h4>
                    <p className="text-gray-400 text-xs">
                      <span className="text-green-400 font-medium">No.</span> All scanning happens 100% locally on your machine.
                      Your file contents never leave your computer. Only when you use the AI features,
                      small text snippets are sent to your chosen AI provider (and you control that).
                    </p>
                  </div>

                  <div className="bg-[#1F1F1F] rounded-lg p-3">
                    <h4 className="text-white font-medium mb-1">What does 4DA do with this information?</h4>
                    <p className="text-gray-400 text-xs">
                      It builds a local understanding of your interests (e.g., "you work with Rust and React")
                      to filter internet content and show you only what's relevant. This context stays in a
                      local database on your machine.
                    </p>
                  </div>

                  <div className="bg-[#1F1F1F] rounded-lg p-3">
                    <h4 className="text-white font-medium mb-1">Can I control what gets scanned?</h4>
                    <p className="text-gray-400 text-xs">
                      Yes! You can add or remove directories anytime in Settings. 4DA automatically
                      ignores sensitive locations like node_modules, .git folders, and hidden system files.
                    </p>
                  </div>

                  <div className="bg-[#1F1F1F] rounded-lg p-3">
                    <h4 className="text-white font-medium mb-1">How long does scanning take?</h4>
                    <p className="text-gray-400 text-xs">
                      Usually a few seconds to a minute depending on how many projects you have.
                      The initial scan is the longest - after that, 4DA only checks for changes.
                    </p>
                  </div>
                </div>
              </details>
            </div>

            <div className="flex justify-between items-center">
              <button
                onClick={prevStep}
                className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
              >
                ← Back
              </button>
              <div className="flex items-center gap-3">
                {!discoveryResult && (
                  <button
                    onClick={nextStep}
                    className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
                  >
                    Skip for now
                  </button>
                )}
                <button
                  onClick={nextStep}
                  className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
                >
                  Continue
                </button>
              </div>
            </div>
          </div>
        )}

        {step === 'interests' && (
          <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
            <h2 className="text-3xl font-semibold text-white mb-2 text-center">Your Interests</h2>
            <p className="text-gray-400 mb-6 text-center">
              Help 4DA understand what to surface. This improves over time as you use the app.
            </p>

            <div className="space-y-5 bg-[#141414] p-6 rounded-lg mb-6">
              {/* Role - simplified */}
              <div>
                <label className="block text-sm text-gray-400 mb-2">
                  What do you do? <span className="text-gray-600">(optional)</span>
                </label>
                <input
                  type="text"
                  value={role}
                  onChange={(e) => setRole(e.target.value)}
                  placeholder="e.g., Software Engineer, Product Manager, Researcher"
                  className="w-full px-4 py-3 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none"
                />
              </div>

              {/* Interests - improved */}
              <div>
                <label className="block text-sm text-gray-400 mb-2">
                  Topics you want to follow
                </label>

                {/* Selected interests first */}
                {interests.length > 0 && (
                  <div className="flex flex-wrap gap-2 mb-3 p-3 bg-[#1F1F1F] rounded-lg border border-[#2A2A2A]">
                    {interests.map((interest) => (
                      <span
                        key={interest}
                        className="px-3 py-1.5 bg-orange-500/20 text-orange-300 rounded-full text-sm flex items-center gap-2 animate-in fade-in duration-200"
                      >
                        {interest}
                        <button
                          onClick={() => removeInterest(interest)}
                          className="hover:text-white text-orange-400/70"
                        >
                          ×
                        </button>
                      </span>
                    ))}
                  </div>
                )}

                {/* Add custom interest */}
                <div className="flex gap-2 mb-4">
                  <input
                    type="text"
                    value={newInterest}
                    onChange={(e) => setNewInterest(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && addInterest()}
                    placeholder="Type a topic and press Enter..."
                    className="flex-1 px-4 py-2 bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none"
                  />
                  <button
                    onClick={addInterest}
                    disabled={!newInterest.trim()}
                    className="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    Add
                  </button>
                </div>

                {/* Suggestions - categorized */}
                <div className="space-y-3">
                  <p className="text-xs text-gray-500">Or quick-add popular topics:</p>
                  <div className="flex flex-wrap gap-2">
                    {suggestedInterests
                      .filter((s) => !interests.includes(s))
                      .slice(0, 10)
                      .map((suggestion) => (
                        <button
                          key={suggestion}
                          onClick={() => setInterests([...interests, suggestion])}
                          className="px-3 py-1.5 bg-[#1F1F1F] text-gray-400 rounded-full text-sm hover:bg-[#2A2A2A] hover:text-white transition-all hover:scale-105"
                        >
                          + {suggestion}
                        </button>
                      ))}
                  </div>
                </div>
              </div>

              {/* Hint */}
              <p className="text-xs text-gray-500 text-center">
                Don't worry about being complete - 4DA learns from your feedback and activity.
              </p>
            </div>

            <div className="flex justify-between items-center">
              <button
                onClick={prevStep}
                className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
              >
                ← Back
              </button>
              <div className="flex items-center gap-3">
                <button
                  onClick={() => {
                    setInterests([]);
                    setRole('');
                    saveInterests();
                  }}
                  className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
                >
                  Skip for now
                </button>
                <button
                  onClick={saveInterests}
                  className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
                >
                  {interests.length > 0 || role ? 'Save & Finish' : 'Finish Setup'}
                </button>
              </div>
            </div>
          </div>
        )}

        {step === 'first-scan' && (
          <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
            <h2 className="text-3xl font-semibold text-white mb-2 text-center">Deep Intelligence Scan</h2>
            <p className="text-gray-400 mb-6 text-center">
              Comprehensive scan of 300-500+ items across all sources to build your personalized feed.
            </p>

            <div className="bg-[#141414] p-6 rounded-lg mb-6">
              {/* Scanning state visualization */}
              {scanProgress?.phase === 'fetching' && (
                <div className="text-center py-8">
                  <div className="w-20 h-20 mx-auto mb-4 relative">
                    <div className="absolute inset-0 rounded-full border-4 border-orange-500/20" />
                    <div className="absolute inset-0 rounded-full border-4 border-orange-500 border-t-transparent animate-spin" style={{ animationDuration: '1.5s' }} />
                    <span className="absolute inset-0 flex items-center justify-center text-3xl">🔬</span>
                  </div>
                  <h3 className="text-white font-medium mb-2">Deep Scan in Progress</h3>
                  <p className="text-sm text-gray-400 mb-3">{scanProgress.message}</p>
                  <div className="flex flex-wrap justify-center gap-2 mt-4">
                    <span className="px-2 py-1 bg-orange-500/20 text-orange-300 text-xs rounded animate-pulse">HN Top</span>
                    <span className="px-2 py-1 bg-orange-500/20 text-orange-300 text-xs rounded animate-pulse delay-100">HN New</span>
                    <span className="px-2 py-1 bg-orange-500/20 text-orange-300 text-xs rounded animate-pulse delay-200">HN Best</span>
                    <span className="px-2 py-1 bg-purple-500/20 text-purple-300 text-xs rounded animate-pulse delay-300">arXiv AI</span>
                    <span className="px-2 py-1 bg-purple-500/20 text-purple-300 text-xs rounded animate-pulse delay-400">arXiv ML</span>
                    <span className="px-2 py-1 bg-blue-500/20 text-blue-300 text-xs rounded animate-pulse delay-500">Reddit</span>
                    <span className="px-2 py-1 bg-green-500/20 text-green-300 text-xs rounded animate-pulse delay-600">GitHub</span>
                  </div>
                  <p className="text-xs text-gray-500 mt-4">This comprehensive scan may take 2-5 minutes</p>
                </div>
              )}

              {scanProgress?.phase === 'scoring' && (
                <div className="text-center py-8">
                  <div className="w-20 h-20 mx-auto mb-4 relative">
                    <div className="absolute inset-0 rounded-full border-4 border-cyan-500/20" />
                    <div className="absolute inset-0 rounded-full border-4 border-cyan-500 border-t-transparent animate-spin" />
                    <span className="absolute inset-0 flex items-center justify-center text-3xl">🤖</span>
                  </div>
                  <h3 className="text-white font-medium mb-2">Analyzing Relevance</h3>
                  <p className="text-sm text-gray-400">{scanProgress.message}</p>
                  <div className="w-48 h-1 bg-[#1F1F1F] rounded-full mx-auto mt-4 overflow-hidden">
                    <div className="h-full bg-gradient-to-r from-cyan-500 to-orange-500 rounded-full animate-pulse" style={{ width: '70%' }} />
                  </div>
                </div>
              )}

              {scanProgress?.phase === 'done' && (
                <div className="py-4">
                  <div className="flex items-center justify-center gap-3 mb-6">
                    <div className="w-12 h-12 bg-green-500/20 rounded-full flex items-center justify-center">
                      <span className="text-2xl">✓</span>
                    </div>
                    <div className="text-left">
                      <h3 className="text-white font-medium">{scanProgress.message}</h3>
                      <p className="text-sm text-gray-400">
                        Analyzed {scanProgress.total} items, {scanProgress.relevant} match your profile
                      </p>
                    </div>
                  </div>

                  {scanProgress.results && scanProgress.results.length > 0 ? (
                    <div className="space-y-2">
                      <p className="text-xs text-gray-500 mb-3">Top results for you:</p>
                      {scanProgress.results.map((result, i) => (
                        <div
                          key={i}
                          className="flex items-center gap-3 p-3 bg-[#1F1F1F] rounded-lg hover:bg-[#2A2A2A] transition-colors"
                        >
                          <span className={`px-2 py-0.5 text-xs rounded ${
                            result.source === 'HN' ? 'bg-orange-500/20 text-orange-300' :
                            result.source === 'arXiv' ? 'bg-purple-500/20 text-purple-300' :
                            'bg-blue-500/20 text-blue-300'
                          }`}>
                            {result.source}
                          </span>
                          <span className="flex-1 text-sm text-gray-300 truncate">{result.title}</span>
                          <span className="text-xs text-green-400 font-mono">{result.score}%</span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="text-center py-4 bg-[#1F1F1F] rounded-lg">
                      <p className="text-gray-400">No highly relevant items found yet.</p>
                      <p className="text-sm text-gray-500 mt-1">
                        4DA will learn your preferences as you give feedback.
                      </p>
                    </div>
                  )}
                </div>
              )}

              {scanProgress?.phase === 'error' && (
                <div className="text-center py-8">
                  <div className="w-16 h-16 mx-auto mb-4 bg-red-500/20 rounded-full flex items-center justify-center">
                    <span className="text-3xl">⚠</span>
                  </div>
                  <h3 className="text-red-300 font-medium mb-2">Scan encountered an issue</h3>
                  <p className="text-sm text-gray-400">{scanProgress.message}</p>
                  <button
                    onClick={runFirstScan}
                    className="mt-4 px-4 py-2 bg-[#1F1F1F] text-gray-300 rounded-lg hover:bg-[#2A2A2A] transition-colors"
                  >
                    Retry Scan
                  </button>
                </div>
              )}

              {!scanProgress && (
                <div className="text-center py-8">
                  <div className="w-16 h-16 mx-auto mb-4 bg-gradient-to-br from-orange-500/20 to-purple-500/20 rounded-full flex items-center justify-center">
                    <span className="text-3xl">🔬</span>
                  </div>
                  <h3 className="text-white font-medium mb-2">Ready for Deep Scan</h3>
                  <p className="text-sm text-gray-400 mb-4 max-w-sm mx-auto">
                    We'll comprehensively scan 300-500+ items from HN (5 categories), arXiv (16 fields),
                    Reddit (40+ subs), and GitHub to build your personalized intelligence feed.
                  </p>
                  <button
                    onClick={runFirstScan}
                    className="px-6 py-3 bg-gradient-to-r from-orange-500 to-orange-600 text-white rounded-lg hover:from-orange-600 hover:to-orange-700 transition-all font-medium shadow-lg shadow-orange-500/20"
                  >
                    Start Deep Scan
                  </button>
                  <p className="text-xs text-gray-500 mt-3">Takes 2-5 minutes for comprehensive results</p>
                </div>
              )}
            </div>

            <div className="flex justify-between items-center">
              <button
                onClick={prevStep}
                disabled={isScanning}
                className="px-6 py-2 text-gray-400 hover:text-white transition-colors disabled:opacity-50"
              >
                ← Back
              </button>
              <div className="flex items-center gap-3">
                {scanProgress?.phase !== 'done' && !isScanning && (
                  <button
                    onClick={async () => {
                      await invoke('mark_onboarding_complete');
                      nextStep();
                    }}
                    className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
                  >
                    Skip for now
                  </button>
                )}
                <button
                  onClick={async () => {
                    await invoke('mark_onboarding_complete');
                    nextStep();
                  }}
                  disabled={isScanning || (scanProgress?.phase !== 'done' && scanProgress?.phase !== 'error')}
                  className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {scanProgress?.phase === 'done' ? 'See My Results →' : 'Continue'}
                </button>
              </div>
            </div>
          </div>
        )}

        {step === 'complete' && (
          <div className={`text-center transition-all duration-500 ${isAnimating ? 'opacity-0 scale-95' : 'opacity-100 scale-100'}`}>
            <div className="w-24 h-24 mx-auto mb-6 bg-gradient-to-br from-green-500/30 to-green-600/20 rounded-full flex items-center justify-center ring-4 ring-green-500/20">
              <span className="text-5xl animate-bounce">🎉</span>
            </div>
            <h2 className="text-3xl font-semibold text-white mb-3">You're All Set!</h2>
            <p className="text-gray-400 mb-8 max-w-md mx-auto">
              4DA found relevant content for you. Here's how to make it even better.
            </p>

            <div className="bg-[#141414] p-5 rounded-lg mb-8 text-left max-w-md mx-auto">
              <h3 className="text-white font-medium mb-4 text-center">Keep improving 4DA</h3>
              <ul className="space-y-4">
                <li className="flex items-start gap-3">
                  <span className="flex-shrink-0 w-7 h-7 bg-green-500/20 rounded-full flex items-center justify-center text-sm text-green-400">
                    ✓
                  </span>
                  <div>
                    <strong className="text-white block text-sm">Save what you like</strong>
                    <span className="text-gray-400 text-sm">Bookmark interesting items to train 4DA</span>
                  </div>
                </li>
                <li className="flex items-start gap-3">
                  <span className="flex-shrink-0 w-7 h-7 bg-red-500/20 rounded-full flex items-center justify-center text-sm text-red-400">
                    ✕
                  </span>
                  <div>
                    <strong className="text-white block text-sm">Dismiss what you don't</strong>
                    <span className="text-gray-400 text-sm">Help 4DA learn what's not relevant</span>
                  </div>
                </li>
                <li className="flex items-start gap-3">
                  <span className="flex-shrink-0 w-7 h-7 bg-orange-500/20 rounded-full flex items-center justify-center text-sm text-orange-400">
                    ⚡
                  </span>
                  <div>
                    <strong className="text-white block text-sm">Let it run</strong>
                    <span className="text-gray-400 text-sm">Background monitoring checks every 30 min</span>
                  </div>
                </li>
              </ul>
            </div>

            <button
              onClick={onComplete}
              className="px-10 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-medium hover:scale-105 active:scale-95"
            >
              Start Using 4DA →
            </button>

            <p className="text-xs text-gray-600 mt-4">
              Press <kbd className="px-1.5 py-0.5 bg-[#1F1F1F] rounded text-gray-400">Settings</kbd> anytime to adjust your preferences
            </p>
          </div>
        )}
      </div>

      {/* Version */}
      <p className="absolute bottom-6 text-xs text-gray-600">Version 0.1.0</p>
    </div>
  );
}
