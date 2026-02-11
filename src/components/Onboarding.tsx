import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { Step, ApiKeyState, OllamaStatus, ScanProgress, PullProgress } from './onboarding/types';
import { WelcomeStep } from './onboarding/WelcomeStep';
import { ApiKeysStep } from './onboarding/ApiKeysStep';
import { ContextStep } from './onboarding/ContextStep';
import { InterestsStep } from './onboarding/InterestsStep';
import { FirstScanStep } from './onboarding/FirstScanStep';
import { CompleteStep } from './onboarding/CompleteStep';

interface OnboardingProps {
  onComplete: () => void;
}

const steps: Step[] = ['welcome', 'api-keys', 'context', 'interests', 'first-scan', 'complete'];

const stepLabels: Record<Step, string> = {
  welcome: 'Welcome',
  'api-keys': 'AI Provider',
  context: 'Context',
  interests: 'Interests',
  'first-scan': 'First Scan',
  complete: 'Ready!',
};

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
  const [isScanning, setIsScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [pullingModels, setPullingModels] = useState(false);
  const [pullProgress, setPullProgress] = useState<Record<string, PullProgress>>({});

  const currentIndex = steps.indexOf(step);

  // Trigger entrance animation
  useEffect(() => {
    setIsAnimating(true);
    const timer = setTimeout(() => setIsAnimating(false), 300);
    return () => clearTimeout(timer);
  }, [step]);

  // Auto-run scan when reaching first-scan step
  useEffect(() => {
    if (step === 'first-scan' && !scanProgress && !isScanning) {
      runFirstScan();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- only trigger scan when step changes to first-scan
  }, [step]);

  const nextStep = () => {
    const next = steps[currentIndex + 1];
    if (next) setStep(next);
  };

  const prevStep = () => {
    const prev = steps[currentIndex - 1];
    if (prev) setStep(prev);
  };

  const pullMissingModels = useCallback(async (status: OllamaStatus) => {
    if (pullingModels) return;
    const needsEmbedding = !status.has_embedding_model;
    const needsLLM = !status.has_llm_model;
    if (!needsEmbedding && !needsLLM) return;

    setPullingModels(true);
    const models: string[] = [];
    if (needsEmbedding) models.push('nomic-embed-text');
    if (needsLLM) models.push('llama3.2');

    // Initialize progress for all models
    const initial: Record<string, PullProgress> = {};
    for (const m of models) initial[m] = { model: m, status: 'waiting', percent: 0, done: false };
    setPullProgress(initial);

    // Listen for progress events
    const unlisten = await listen<PullProgress>('ollama-pull-progress', (event) => {
      setPullProgress((prev) => ({
        ...prev,
        [event.payload.model]: event.payload,
      }));
    });

    try {
      // Pull sequentially (embedding first - smaller & always needed)
      for (const model of models) {
        setPullProgress((prev) => ({
          ...prev,
          [model]: { model, status: 'downloading', percent: 0, done: false },
        }));
        await invoke('pull_ollama_model', {
          model,
          baseUrl: status.base_url || null,
        });
        setPullProgress((prev) => ({
          ...prev,
          [model]: { model, status: 'success', percent: 100, done: true },
        }));
      }

      // Re-check Ollama status with retry - Ollama needs time to load the model after pull
      let refreshed: OllamaStatus | null = null;
      for (let attempt = 0; attempt < 5; attempt++) {
        // Wait before checking (model needs time to load into memory)
        await new Promise(r => setTimeout(r, attempt === 0 ? 2000 : 3000));
        try {
          const status = await invoke<OllamaStatus>('check_ollama_status', { baseUrl: null });
          if (status.running && status.models.length > 0) {
            refreshed = status;
            break;
          }
        } catch {
          // Ollama may still be loading, retry
        }
      }
      if (refreshed) {
        setOllamaStatus(refreshed);
        const llmModel = refreshed.models.find((m: string) => !m.startsWith('nomic-embed-text'));
        setSelectedOllamaModel(llmModel || refreshed.models[0]);
      }
    } catch (e) {
      setError(`Model download failed: ${e}`);
    } finally {
      unlisten();
      setPullingModels(false);
    }
  }, [pullingModels]);

  const checkOllamaStatus = useCallback(async () => {
    setIsCheckingOllama(true);
    setError(null);
    try {
      const status = await invoke<OllamaStatus>('check_ollama_status', { baseUrl: null });
      setOllamaStatus(status);
      if (status.running && status.models.length > 0) {
        const llmModel = status.models.find((m) => !m.startsWith('nomic-embed-text'));
        setSelectedOllamaModel(llmModel || status.models[0]);
      }
      // Auto-pull missing models when Ollama is running
      if (status.running && (!status.has_embedding_model || !status.has_llm_model) && apiKeys.provider === 'ollama') {
        pullMissingModels(status);
      }
    } catch {
      setOllamaStatus({ running: false, version: null, models: [], base_url: 'http://localhost:11434' });
    }
    setIsCheckingOllama(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- pullMissingModels is stable
  }, [apiKeys.provider]);

  // Auto-check Ollama when provider is selected
  useEffect(() => {
    if (apiKeys.provider === 'ollama') {
      checkOllamaStatus();
    }
  }, [apiKeys.provider, checkOllamaStatus]);

  const testApiConnection = async () => {
    try {
      setIsTesting(true);
      setError(null);
      setTestResult(null);

      const provider = apiKeys.provider;
      const apiKey = provider === 'anthropic' ? apiKeys.anthropic :
                     provider === 'openai' ? apiKeys.openai : '';
      const ollamaModel = selectedOllamaModel || ollamaStatus?.models?.find((m) => !m.startsWith('nomic-embed-text')) || ollamaStatus?.models?.[0];
      if (provider === 'ollama' && !ollamaModel) {
        setTestResult({ success: false, message: 'No models available. Wait for model download to complete.' });
        setIsTesting(false);
        return;
      }
      const model = provider === 'anthropic' ? 'claude-3-haiku-20240307' :
                    provider === 'openai' ? 'gpt-4o-mini' :
                    (ollamaModel || 'llama3.2');
      const baseUrl = provider === 'ollama' ? (ollamaStatus?.base_url || 'http://localhost:11434') : null;
      const openaiApiKey = provider !== 'openai' ? apiKeys.openai : null;

      await invoke('set_llm_provider', { provider, apiKey, model, baseUrl, openaiApiKey });

      // Race the test against a timeout (90s — generous for Ollama cold starts)
      const timeoutMs = provider === 'ollama' ? 90_000 : 30_000;
      const testPromise = invoke<{ success: boolean; message: string }>('test_llm_connection');
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(
          provider === 'ollama'
            ? `Ollama did not respond within ${timeoutMs / 1000}s. The model may be too large for your hardware, or Ollama may be stuck. Try restarting Ollama.`
            : `Connection timed out after ${timeoutMs / 1000}s. Check your internet connection.`,
        )), timeoutMs),
      );

      const result = await Promise.race([testPromise, timeoutPromise]);
      setTestResult(result);

      if (!result.success) {
        const msg = result.message.toLowerCase();
        let hint = '';
        if (msg.includes('401') || msg.includes('invalid') || msg.includes('unauthorized')) {
          hint = 'Check that your API key is correct and has not expired.';
        } else if (msg.includes('429') || msg.includes('rate')) {
          hint = "You've hit a rate limit. Wait a moment and try again.";
        } else if (msg.includes('not found') && provider === 'ollama') {
          hint = `Run: ollama pull ${model}`;
        } else if (msg.includes('memory') || msg.includes('oom') || msg.includes('cuda')) {
          hint = 'Not enough GPU memory. Try a smaller model like phi3:mini.';
        } else if (msg.includes('connect') || msg.includes('refused')) {
          hint = provider === 'ollama' ? 'Make sure Ollama is running (ollama serve).' : 'Check your internet connection.';
        }
        if (hint) setError(hint);
      }
    } catch (e) {
      const msg = String(e);
      setTestResult({ success: false, message: msg });
      if (msg.includes('did not respond')) {
        setError(msg);
      } else if (apiKeys.provider === 'ollama') {
        setError('Ollama connection test failed. Make sure Ollama is running and the model is installed.');
      } else {
        setError('Connection test failed. Check your settings and try again.');
      }
    } finally {
      setIsTesting(false);
    }
  };

  const saveApiKeys = async () => {
    try {
      setError(null);
      const provider = apiKeys.provider;
      const apiKey = provider === 'anthropic' ? apiKeys.anthropic :
                     provider === 'openai' ? apiKeys.openai : '';
      const ollamaModel = selectedOllamaModel || ollamaStatus?.models?.find((m) => !m.startsWith('nomic-embed-text')) || ollamaStatus?.models?.[0];
      if (provider === 'ollama' && !ollamaModel) {
        setError('No Ollama models available. Please wait for model download to finish.');
        return;
      }
      const model = provider === 'anthropic' ? 'claude-3-haiku-20240307' :
                    provider === 'openai' ? 'gpt-4o-mini' :
                    (ollamaModel || 'llama3.2');
      const baseUrl = provider === 'ollama' ? (ollamaStatus?.base_url || 'http://localhost:11434') : null;
      const openaiApiKey = provider !== 'openai' ? apiKeys.openai : null;

      await invoke('set_llm_provider', { provider, apiKey, model, baseUrl, openaiApiKey });

      if (apiKeys.xApiKey.trim()) {
        try {
          const result = await invoke('set_x_api_key', { key: apiKeys.xApiKey.trim() }) as { validated?: boolean; warning?: string };
          if (result?.warning) {
            console.warn('X API key warning:', result.warning);
          }
        } catch (xErr) {
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

  const skipApiSetup = async () => {
    try {
      await invoke('set_llm_provider', {
        provider: 'ollama',
        apiKey: '',
        model: 'llama3.2',
        baseUrl: 'http://localhost:11434',
      });
      nextStep();
    } catch {
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
      if (role) {
        await invoke('set_user_role', { role });
      }
      for (const interest of interests) {
        await invoke('add_interest', { topic: interest });
      }
      nextStep();
    } catch (e) {
      setError(`Failed to save: ${e}`);
    }
  };

  const runFirstScan = async () => {
    try {
      setIsScanning(true);
      setScanProgress({
        phase: 'fetching',
        message: 'Deep scanning HN, arXiv, Reddit, GitHub, RSS, YouTube, Twitter...',
      });

      await invoke('run_deep_initial_scan');
      setScanProgress({ phase: 'scoring', message: 'Analyzing hundreds of items for relevance...' });

      const results = await invoke<Array<{
        id: number;
        title: string;
        url: string | null;
        top_score: number;
        source_type: string;
        relevant: boolean;
      }>>('compute_relevance');

      const relevant = results.filter((r) => r.relevant || r.top_score >= 0.3);
      const topPicks = results.filter((r) => r.top_score >= 0.6);
      const topResults = relevant
        .sort((a, b) => b.top_score - a.top_score)
        .slice(0, 8)
        .map((r) => ({
          title: r.title,
          score: Math.round(r.top_score * 100),
          source: ({ hackernews: 'HN', arxiv: 'arXiv', reddit: 'Reddit', github: 'GitHub',
                     rss: 'RSS', youtube: 'YouTube', twitter: 'Twitter', producthunt: 'PH',
                     lobsters: 'Lobsters', devto: 'Dev.to',
                  } as Record<string, string>)[r.source_type] || r.source_type,
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

  const handleFirstScanComplete = async () => {
    await invoke('mark_onboarding_complete');
    nextStep();
  };

  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-[#0A0A0A] p-8">
      {/* Progress indicator */}
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
              {i < currentIndex ? '\u2713' : i + 1}
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

      {/* Error display */}
      {error && (
        <div className="absolute top-24 max-w-md bg-red-900/30 border border-red-500/30 text-red-200 px-4 py-3 rounded-lg animate-in fade-in slide-in-from-top-2 duration-200">
          <div className="flex items-start gap-2">
            <span className="text-red-400 flex-shrink-0">&#x26a0;</span>
            <div>
              <div className="font-medium text-red-300 text-sm">Something went wrong</div>
              <div className="text-xs text-red-200/80 mt-1">{error}</div>
            </div>
            <button
              onClick={() => setError(null)}
              className="text-red-400 hover:text-red-300 ml-2"
            >
              &times;
            </button>
          </div>
        </div>
      )}

      {/* Step content */}
      <div className="max-w-2xl w-full">
        {step === 'welcome' && (
          <WelcomeStep isAnimating={isAnimating} onNext={nextStep} />
        )}

        {step === 'api-keys' && (
          <ApiKeysStep
            isAnimating={isAnimating}
            apiKeys={apiKeys}
            setApiKeys={setApiKeys}
            ollamaStatus={ollamaStatus}
            isCheckingOllama={isCheckingOllama}
            checkOllamaStatus={checkOllamaStatus}
            selectedOllamaModel={selectedOllamaModel}
            setSelectedOllamaModel={setSelectedOllamaModel}
            isTesting={isTesting}
            testResult={testResult}
            setTestResult={setTestResult}
            error={error}
            setError={setError}
            onTest={testApiConnection}
            onSave={saveApiKeys}
            onSkip={skipApiSetup}
            onBack={prevStep}
            pullingModels={pullingModels}
            pullProgress={pullProgress}
          />
        )}

        {step === 'context' && (
          <ContextStep
            isAnimating={isAnimating}
            isDiscovering={isDiscovering}
            discoveryResult={discoveryResult}
            onDiscovery={runAutoDiscovery}
            onNext={nextStep}
            onBack={prevStep}
          />
        )}

        {step === 'interests' && (
          <InterestsStep
            isAnimating={isAnimating}
            role={role}
            setRole={setRole}
            interests={interests}
            setInterests={setInterests}
            newInterest={newInterest}
            setNewInterest={setNewInterest}
            onSave={saveInterests}
            onBack={prevStep}
          />
        )}

        {step === 'first-scan' && (
          <FirstScanStep
            isAnimating={isAnimating}
            isScanning={isScanning}
            scanProgress={scanProgress}
            onRunScan={runFirstScan}
            onComplete={handleFirstScanComplete}
            onBack={prevStep}
          />
        )}

        {step === 'complete' && (
          <CompleteStep isAnimating={isAnimating} onComplete={onComplete} />
        )}
      </div>

      {/* Version */}
      <p className="absolute bottom-6 text-xs text-gray-600">Version 1.0.0</p>
    </div>
  );
}
