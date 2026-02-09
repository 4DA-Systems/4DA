import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type { Step, ApiKeyState, OllamaStatus, ScanProgress } from './onboarding/types';
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

      await invoke('set_llm_provider', { provider, apiKey, model, baseUrl, openaiApiKey });
      const result = await invoke<{ success: boolean; message: string }>('test_llm_connection');
      setTestResult(result);

      if (!result.success) {
        let hint = '';
        if (result.message.includes('401') || result.message.includes('invalid')) {
          hint = 'Check that your API key is correct and has not expired.';
        } else if (result.message.includes('429') || result.message.includes('rate')) {
          hint = "You've hit a rate limit. Wait a moment and try again.";
        } else if (result.message.includes('network') || result.message.includes('connect')) {
          hint = 'Check your internet connection.';
        }
        if (hint) setError(hint);
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
      const provider = apiKeys.provider;
      const apiKey = provider === 'anthropic' ? apiKeys.anthropic :
                     provider === 'openai' ? apiKeys.openai : '';
      const model = provider === 'anthropic' ? 'claude-3-haiku-20240307' :
                    provider === 'openai' ? 'gpt-4o-mini' :
                    (selectedOllamaModel || ollamaStatus?.models[0] || 'llama3.2');
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
