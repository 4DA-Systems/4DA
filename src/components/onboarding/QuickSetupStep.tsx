import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { OllamaStatus, PullProgress } from './types';
import { SetupAIProvider } from './setup-ai-provider';
import { SetupProjects } from './setup-projects';
import { SetupStack } from './setup-stack';
import { SetupInterests } from './setup-interests';
import { SetupLocale } from './setup-locale';

interface QuickSetupStepProps {
  isAnimating: boolean;
  onComplete: () => void;
  onBack: () => void;
}

interface SuggestedInterest {
  topic: string;
  source: string;
  confidence: number;
  already_declared: boolean;
}

const roles = ['Developer', 'Security', 'DevOps', 'Data', 'Manager'];

const fallbackSuggestions = [
  'Machine Learning', 'Rust', 'TypeScript', 'Web Development',
  'DevOps', 'Security', 'Startups', 'Open Source', 'AI/LLM',
  'Mobile Development', 'Cloud Infrastructure', 'Data Engineering',
];

export function QuickSetupStep({ isAnimating, onComplete, onBack }: QuickSetupStepProps) {
  const { t } = useTranslation();

  // Section collapse state
  const [aiOpen, setAiOpen] = useState(true);
  const [projectsOpen, setProjectsOpen] = useState(false);
  const [stacksOpen, setStacksOpen] = useState(false);
  const [interestsOpen, setInterestsOpen] = useState(false);
  const [localeOpen, setLocaleOpen] = useState(false);
  const [localeConfigured, setLocaleConfigured] = useState(false);

  // Stack Intelligence state
  const [selectedStacks, setSelectedStacks] = useState<string[]>([]);

  // AI Provider state
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus | null>(null);
  const [provider, setProvider] = useState<'anthropic' | 'openai' | 'ollama'>('ollama');
  const [apiKey, setApiKey] = useState('');
  const [pullingModels, setPullingModels] = useState(false);
  const [pullProgress, setPullProgress] = useState<Record<string, PullProgress>>({});
  const [aiConfigured, setAiConfigured] = useState(false);

  // Projects state
  const [detectedTech, setDetectedTech] = useState<string[]>([]);
  const [discoveryDone, setDiscoveryDone] = useState(false);

  // Interests state
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [interests, setInterests] = useState<string[]>([]);
  const [newInterest, setNewInterest] = useState('');
  const [role, setRole] = useState('Developer');

  const [error, setError] = useState<string | null>(null);

  // --- AI Provider auto-detect ---
  const pullMissingModels = useCallback(async (status: OllamaStatus) => {
    const needsEmbedding = !status.has_embedding_model;
    const needsLLM = !status.has_llm_model;
    if (!needsEmbedding && !needsLLM) return;

    setPullingModels(true);
    const models: string[] = [];
    if (needsEmbedding) models.push('nomic-embed-text');
    if (needsLLM) models.push('llama3.2');

    const initial: Record<string, PullProgress> = {};
    for (const m of models) initial[m] = { model: m, status: 'waiting', percent: 0, done: false };
    setPullProgress(initial);

    const unlisten = await listen<PullProgress>('ollama-pull-progress', (event) => {
      setPullProgress((prev) => ({
        ...prev,
        [event.payload.model]: event.payload,
      }));
    });

    try {
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

      // Re-check status after pull
      let refreshed: OllamaStatus | null = null;
      for (let attempt = 0; attempt < 5; attempt++) {
        await new Promise(r => setTimeout(r, attempt === 0 ? 2000 : 3000));
        try {
          const s = await invoke<OllamaStatus>('check_ollama_status', { baseUrl: null });
          if (s.running && s.models.length > 0) {
            refreshed = s;
            break;
          }
        } catch { /* retry */ }
      }
      if (refreshed) {
        setOllamaStatus(refreshed);
        setAiConfigured(true);
      }
    } catch (e) {
      setError(`Model download failed: ${e}`);
    } finally {
      unlisten();
      setPullingModels(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const status = await invoke<OllamaStatus>('check_ollama_status', { baseUrl: null });
        if (cancelled) return;
        setOllamaStatus(status);

        if (status.running && status.has_embedding_model && status.has_llm_model) {
          // Ollama ready with models
          setProvider('ollama');
          setAiConfigured(true);
          setAiOpen(false);
          setProjectsOpen(true);
        } else if (status.running) {
          // Ollama running but missing models - auto-pull
          setProvider('ollama');
          pullMissingModels(status);
        }
        // If Ollama not running, leave section open for manual config
      } catch {
        setOllamaStatus({ running: false, version: null, models: [], base_url: 'http://localhost:11434' });
      }
    })();
    return () => { cancelled = true; };
  }, [pullMissingModels]);

  // --- Auto-discover projects ---
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await invoke<{
          success: boolean;
          scan_result?: {
            combined?: { topics?: string[] };
          };
        }>('ace_auto_discover');
        if (cancelled) return;
        setDiscoveryDone(true);

        // Extract detected tech from scan result
        const topics = result.scan_result?.combined?.topics || [];
        if (topics.length > 0) {
          setDetectedTech(topics.slice(0, 12));
        }
      } catch {
        // Discovery failed silently - not critical
        setDiscoveryDone(true);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // --- Load suggested interests after discovery ---
  useEffect(() => {
    if (!discoveryDone) return;
    let cancelled = false;
    (async () => {
      try {
        const result = await invoke<SuggestedInterest[]>('ace_get_suggested_interests');
        if (cancelled) return;
        const topics = result
          .filter(s => !s.already_declared)
          .map(s => s.topic)
          .slice(0, 12);
        const finalSuggestions = topics.length > 0 ? topics : fallbackSuggestions;
        setSuggestions(finalSuggestions);
        // Auto-populate top suggestions as default interests (user removes what doesn't fit)
        setInterests(prev => prev.length === 0 ? finalSuggestions.slice(0, 5) : prev);
      } catch {
        setSuggestions(fallbackSuggestions);
        // Ensure first-run always has some interests for scoring
        setInterests(prev => prev.length === 0 ? fallbackSuggestions.slice(0, 5) : prev);
      }
    })();
    return () => { cancelled = true; };
  }, [discoveryDone]);

  // --- Handlers ---
  const removeTag = (tag: string) => {
    setDetectedTech(prev => prev.filter(t => t !== tag));
  };

  const addInterest = () => {
    const trimmed = newInterest.trim();
    if (trimmed && !interests.includes(trimmed)) {
      setInterests(prev => [...prev, trimmed]);
      setNewInterest('');
    }
  };

  const toggleInterest = (topic: string) => {
    if (interests.includes(topic)) {
      setInterests(prev => prev.filter(i => i !== topic));
    } else {
      setInterests(prev => [...prev, topic]);
    }
  };

  const handleProviderChange = (p: 'anthropic' | 'openai' | 'ollama') => {
    setProvider(p);
    setAiConfigured(p === 'ollama' && !!ollamaStatus?.running && !!ollamaStatus.has_embedding_model && !!ollamaStatus.has_llm_model);
  };

  const handleApiKeyChange = (key: string) => {
    setApiKey(key);
    setAiConfigured(key.trim().length > 0);
  };

  const handleContinue = async () => {
    setError(null);
    try {
      // Save provider config — guard against saving 'ollama' when it's not running
      if (provider === 'ollama') {
        if (ollamaStatus?.running) {
          const ollamaModel = ollamaStatus.models?.find(m => !m.startsWith('nomic-embed-text')) || 'llama3.2';
          await invoke('set_llm_provider', {
            provider: 'ollama',
            apiKey: '',
            model: ollamaModel,
            baseUrl: ollamaStatus.base_url || 'http://localhost:11434',
            openaiApiKey: null,
          });
        } else {
          // Ollama selected but not running — save as 'none' (keyword-only mode)
          await invoke('set_llm_provider', {
            provider: 'none',
            apiKey: '',
            model: '',
            baseUrl: null,
            openaiApiKey: null,
          });
        }
      } else {
        const model = provider === 'anthropic' ? 'claude-3-haiku-20240307' : 'gpt-4o-mini';
        await invoke('set_llm_provider', {
          provider,
          apiKey,
          model,
          baseUrl: null,
          openaiApiKey: provider === 'openai' ? apiKey : null,
        });
      }

      // Save role
      if (role) {
        await invoke('set_user_role', { role });
      }

      // Save interests (fallback to detected tech or defaults if empty)
      const interestsToSave = interests.length > 0
        ? interests
        : detectedTech.length > 0
          ? detectedTech.slice(0, 5)
          : fallbackSuggestions.slice(0, 3);
      for (const interest of interestsToSave) {
        await invoke('add_interest', { topic: interest });
      }

      // Save detected technologies as declared tech stack
      for (const tech of detectedTech) {
        await invoke('add_tech_stack', { technology: tech });
      }

      // Save selected stack profiles
      if (selectedStacks.length > 0) {
        await invoke('set_selected_stacks', { profileIds: selectedStacks });
      }

      onComplete();
    } catch (e) {
      setError(`Failed to save settings: ${e}`);
    }
  };

  // --- Section header component ---
  const SectionHeader = ({
    title,
    subtitle,
    isOpen,
    onToggle,
    done,
  }: {
    title: string;
    subtitle: string;
    isOpen: boolean;
    onToggle: () => void;
    done: boolean;
  }) => (
    <button
      onClick={onToggle}
      aria-expanded={isOpen}
      className="w-full flex items-center justify-between p-4 bg-bg-secondary rounded-lg border border-border hover:border-[#3A3A3A] transition-colors"
    >
      <div className="flex items-center gap-3">
        {done ? (
          <span className="w-6 h-6 bg-green-500/20 rounded-full flex items-center justify-center text-green-400 text-xs">
            &#x2713;
          </span>
        ) : (
          <span className="w-6 h-6 bg-bg-tertiary rounded-full flex items-center justify-center text-gray-500 text-xs">
            &#x25CB;
          </span>
        )}
        <div className="text-left">
          <div className="text-white font-medium text-sm">{title}</div>
          <div className="text-gray-500 text-xs">{subtitle}</div>
        </div>
      </div>
      <span className={`text-gray-500 text-xs transition-transform ${isOpen ? 'rotate-180' : ''}`}>
        &#x25BC;
      </span>
    </button>
  );

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">{t('onboarding.setup.title')}</h2>
      <p className="text-gray-400 mb-6 text-center">
        {t('onboarding.setup.subtitle')}
      </p>

      {error && (
        <div role="alert" className="mb-4 p-3 bg-red-900/30 border border-red-500/30 rounded-lg text-sm text-red-200 flex items-start gap-2">
          <span className="text-red-400 flex-shrink-0" aria-hidden="true">&#x26a0;</span>
          <span className="flex-1">{error}</span>
          <button onClick={() => setError(null)} aria-label="Dismiss error" className="text-red-400 hover:text-red-300">&times;</button>
        </div>
      )}

      <div className="space-y-3 mb-6 max-h-[55vh] overflow-y-auto pr-1">
        {/* Section 1: AI Provider */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.aiProvider')}
            subtitle={aiConfigured
              ? (provider === 'ollama' ? t('onboarding.setup.localAiReady') : `${provider === 'anthropic' ? 'Anthropic' : 'OpenAI'} ${t('onboarding.setup.configured')}`)
              : t('onboarding.setup.autoDetecting')}
            isOpen={aiOpen}
            onToggle={() => setAiOpen(!aiOpen)}
            done={aiConfigured}
          />
          {aiOpen && (
            <SetupAIProvider
              ollamaStatus={ollamaStatus}
              provider={provider}
              apiKey={apiKey}
              pullingModels={pullingModels}
              pullProgress={pullProgress}
              onProviderChange={handleProviderChange}
              onApiKeyChange={handleApiKeyChange}
            />
          )}
        </div>

        {/* Section 2: Your Projects */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourProjects')}
            subtitle={discoveryDone
              ? (detectedTech.length > 0 ? t('onboarding.setup.techDetected', { count: detectedTech.length }) : t('onboarding.setup.discoveryComplete'))
              : t('onboarding.setup.scanning')}
            isOpen={projectsOpen}
            onToggle={() => setProjectsOpen(!projectsOpen)}
            done={discoveryDone}
          />
          {projectsOpen && (
            <SetupProjects
              discoveryDone={discoveryDone}
              detectedTech={detectedTech}
              onRemoveTag={removeTag}
            />
          )}
        </div>

        {/* Section 3: Your Stack */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourStack')}
            subtitle={selectedStacks.length > 0 ? t('onboarding.setup.profilesSelected', { count: selectedStacks.length }) : t('onboarding.setup.autoDetecting')}
            isOpen={stacksOpen}
            onToggle={() => setStacksOpen(!stacksOpen)}
            done={selectedStacks.length > 0}
          />
          <div style={{ display: stacksOpen ? undefined : 'none' }}>
            <SetupStack
              selectedStacks={selectedStacks}
              onSelectionChange={setSelectedStacks}
            />
          </div>
        </div>

        {/* Section: Your Region */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourRegion')}
            subtitle={localeConfigured ? t('onboarding.setup.configured') : t('onboarding.setup.autoDetected')}
            isOpen={localeOpen}
            onToggle={() => setLocaleOpen(!localeOpen)}
            done={localeConfigured}
          />
          <div style={{ display: localeOpen ? undefined : 'none' }}>
            <SetupLocale onLocaleChange={() => setLocaleConfigured(true)} />
          </div>
        </div>

        {/* Section 4: Your Interests */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourInterests')}
            subtitle={interests.length > 0 ? t('onboarding.setup.topicsSelected', { count: interests.length }) : t('onboarding.setup.suggestedForYou')}
            isOpen={interestsOpen}
            onToggle={() => setInterestsOpen(!interestsOpen)}
            done={interests.length > 0}
          />
          <div style={{ display: interestsOpen ? undefined : 'none' }}>
            <SetupInterests
              roles={roles}
              role={role}
              interests={interests}
              newInterest={newInterest}
              suggestions={suggestions}
              onRoleChange={setRole}
              onNewInterestChange={setNewInterest}
              onAddInterest={addInterest}
              onToggleInterest={toggleInterest}
            />
          </div>
        </div>
      </div>

      {/* Navigation */}
      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
        >
          &larr; {t('onboarding.nav.back')}
        </button>
        <div className="flex flex-col items-end gap-1.5">
          <button
            onClick={handleContinue}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
          >
            {t('onboarding.setup.enter4DA')}
          </button>
          {pullingModels && (
            <button
              onClick={handleContinue}
              className="text-xs text-gray-500 hover:text-gray-300 transition-colors"
            >
              {t('onboarding.setup.skipDownload')}
            </button>
          )}
          {!pullingModels && (
            <p className="text-[11px] text-gray-600">
              {t('onboarding.setup.allSectionsOptional')}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
