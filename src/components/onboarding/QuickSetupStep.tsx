import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { OllamaStatus, PullProgress } from './types';

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
  // Section collapse state
  const [aiOpen, setAiOpen] = useState(true);
  const [projectsOpen, setProjectsOpen] = useState(false);
  const [interestsOpen, setInterestsOpen] = useState(false);

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
        setSuggestions(topics.length > 0 ? topics : fallbackSuggestions);
      } catch {
        setSuggestions(fallbackSuggestions);
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

  const handleContinue = async () => {
    setError(null);
    try {
      // Save provider config
      if (provider === 'ollama') {
        const ollamaModel = ollamaStatus?.models?.find(m => !m.startsWith('nomic-embed-text')) || 'llama3.2';
        await invoke('set_llm_provider', {
          provider: 'ollama',
          apiKey: '',
          model: ollamaModel,
          baseUrl: ollamaStatus?.base_url || 'http://localhost:11434',
          openaiApiKey: null,
        });
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

      // Save interests
      for (const interest of interests) {
        await invoke('add_interest', { topic: interest });
      }

      // Save detected technologies as declared tech stack
      for (const tech of detectedTech) {
        await invoke('add_tech_stack', { technology: tech });
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
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">Quick Setup</h2>
      <p className="text-gray-400 mb-6 text-center">
        Configure your AI and interests. Everything stays local and private.
      </p>

      {error && (
        <div className="mb-4 p-3 bg-red-900/30 border border-red-500/30 rounded-lg text-sm text-red-200 flex items-start gap-2">
          <span className="text-red-400 flex-shrink-0">&#x26a0;</span>
          <span className="flex-1">{error}</span>
          <button onClick={() => setError(null)} className="text-red-400 hover:text-red-300">&times;</button>
        </div>
      )}

      <div className="space-y-3 mb-6 max-h-[55vh] overflow-y-auto pr-1">
        {/* Section 1: AI Provider */}
        <div>
          <SectionHeader
            title="AI Provider"
            subtitle={aiConfigured
              ? (provider === 'ollama' ? 'Local AI Ready' : `${provider === 'anthropic' ? 'Anthropic' : 'OpenAI'} configured`)
              : 'Auto-detecting...'}
            isOpen={aiOpen}
            onToggle={() => setAiOpen(!aiOpen)}
            done={aiConfigured}
          />
          {aiOpen && (
            <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-3">
              {/* Ollama detected and ready */}
              {ollamaStatus?.running && ollamaStatus.has_embedding_model && ollamaStatus.has_llm_model && provider === 'ollama' && (
                <div className="p-3 bg-green-900/20 border border-green-500/30 rounded-lg text-sm text-green-300 flex items-center gap-2">
                  <span className="text-green-500">&#x2713;</span>
                  Local AI Ready - no API keys needed
                </div>
              )}

              {/* Pulling models */}
              {pullingModels && (
                <div className="p-4 border border-orange-500/30 rounded-lg space-y-3">
                  <div className="flex items-center gap-2 text-sm text-orange-300">
                    <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                    Installing models for local AI...
                  </div>
                  {Object.entries(pullProgress).map(([model, p]) => (
                    <div key={model} className="space-y-1">
                      <div className="flex items-center justify-between text-xs">
                        <span className="text-gray-300 font-mono">{model}</span>
                        <span className="text-gray-500">
                          {p.done ? 'Complete' : p.status || `${p.percent}%`}
                        </span>
                      </div>
                      <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                        <div
                          className={`h-full rounded-full transition-all duration-300 ${
                            p.done ? 'bg-green-500' : 'bg-orange-500'
                          }`}
                          style={{ width: `${p.done ? 100 : p.percent}%` }}
                        />
                      </div>
                    </div>
                  ))}
                  <p className="text-xs text-gray-500">
                    This may take a few minutes depending on your connection.
                  </p>
                </div>
              )}

              {/* Provider selector (shown when Ollama not ready) */}
              {!(ollamaStatus?.running && ollamaStatus.has_embedding_model && ollamaStatus.has_llm_model) && !pullingModels && (
                <>
                  <div className="grid grid-cols-3 gap-2">
                    {(['ollama', 'anthropic', 'openai'] as const).map((p) => (
                      <button
                        key={p}
                        onClick={() => {
                          setProvider(p);
                          setAiConfigured(p === 'ollama' && !!ollamaStatus?.running && !!ollamaStatus.has_embedding_model && !!ollamaStatus.has_llm_model);
                        }}
                        className={`p-3 rounded-lg text-center transition-all ${
                          provider === p
                            ? 'bg-orange-500/20 border-2 border-orange-500'
                            : 'bg-bg-tertiary border-2 border-transparent hover:border-border'
                        }`}
                      >
                        <div className="text-sm font-medium text-white">
                          {p === 'ollama' ? 'Ollama' : p === 'anthropic' ? 'Anthropic' : 'OpenAI'}
                        </div>
                        <div className="text-xs text-gray-500 mt-1">
                          {p === 'ollama' ? 'Local' : p === 'anthropic' ? 'Claude' : 'GPT-4o'}
                        </div>
                      </button>
                    ))}
                  </div>

                  {/* API key input for cloud providers */}
                  {(provider === 'anthropic' || provider === 'openai') && (
                    <div>
                      <div className="flex items-center justify-between mb-2">
                        <label className="text-xs text-gray-500">
                          {provider === 'anthropic' ? 'Anthropic' : 'OpenAI'} API Key
                        </label>
                        <a
                          href={provider === 'anthropic'
                            ? 'https://console.anthropic.com/settings/keys'
                            : 'https://platform.openai.com/api-keys'}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-xs text-orange-500 hover:underline"
                        >
                          Get key &rarr;
                        </a>
                      </div>
                      <input
                        type="password"
                        value={apiKey}
                        onChange={(e) => {
                          setApiKey(e.target.value);
                          setAiConfigured(e.target.value.trim().length > 0);
                        }}
                        placeholder={provider === 'anthropic' ? 'sk-ant-api03-...' : 'sk-proj-...'}
                        className="w-full px-4 py-3 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none font-mono text-sm"
                      />
                    </div>
                  )}

                  {/* Ollama not running hint */}
                  {provider === 'ollama' && !ollamaStatus?.running && (
                    <div className="text-yellow-400 text-sm p-3 bg-bg-tertiary rounded-lg">
                      Ollama not detected.{' '}
                      <a href="https://ollama.ai" target="_blank" rel="noopener noreferrer" className="text-orange-500 hover:underline">
                        Install Ollama
                      </a>
                      {' '}for free local AI, or choose a cloud provider above.
                    </div>
                  )}
                </>
              )}

              <p className="text-xs text-gray-500">
                No AI? 4DA still works with keyword-only matching. Add keys later in Settings.
              </p>
            </div>
          )}
        </div>

        {/* Section 2: Your Projects */}
        <div>
          <SectionHeader
            title="Your Projects"
            subtitle={discoveryDone
              ? (detectedTech.length > 0 ? `${detectedTech.length} technologies detected` : 'Discovery complete')
              : 'Scanning...'}
            isOpen={projectsOpen}
            onToggle={() => setProjectsOpen(!projectsOpen)}
            done={discoveryDone}
          />
          {projectsOpen && (
            <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border">
              {!discoveryDone ? (
                <div className="flex items-center gap-2 text-sm text-gray-400 py-2">
                  <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
                  Scanning your projects...
                </div>
              ) : detectedTech.length > 0 ? (
                <div>
                  <p className="text-xs text-gray-500 mb-3">Detected from your local projects:</p>
                  <div className="flex flex-wrap gap-2">
                    {detectedTech.map((tech) => (
                      <span
                        key={tech}
                        className="px-3 py-1.5 bg-green-500/10 text-green-400 rounded-lg border border-green-500/20 text-sm flex items-center gap-2"
                      >
                        {tech}
                        <button
                          onClick={() => removeTag(tech)}
                          className="hover:text-white text-green-400/70"
                        >
                          &times;
                        </button>
                      </span>
                    ))}
                  </div>
                </div>
              ) : (
                <p className="text-sm text-gray-400 py-2">
                  No specific technologies detected. 4DA will learn from your activity.
                </p>
              )}
              <p className="text-xs text-gray-500 mt-3">
                Manage directories anytime in Settings.
              </p>
            </div>
          )}
        </div>

        {/* Section 3: Your Interests */}
        <div>
          <SectionHeader
            title="Your Interests"
            subtitle={interests.length > 0 ? `${interests.length} topics selected` : 'Suggested for you'}
            isOpen={interestsOpen}
            onToggle={() => setInterestsOpen(!interestsOpen)}
            done={interests.length > 0}
          />
          {interestsOpen && (
            <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-3">
              {/* Role selector */}
              <div>
                <label className="block text-xs text-gray-500 mb-2">Your role</label>
                <select
                  value={role}
                  onChange={(e) => setRole(e.target.value)}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-white text-sm focus:border-orange-500 focus:outline-none"
                >
                  {roles.map((r) => (
                    <option key={r} value={r}>{r}</option>
                  ))}
                </select>
              </div>

              {/* Selected interests */}
              {interests.length > 0 && (
                <div className="flex flex-wrap gap-2 p-3 bg-bg-tertiary rounded-lg border border-border">
                  {interests.map((interest) => (
                    <span
                      key={interest}
                      className="px-3 py-1.5 bg-orange-500/20 text-orange-300 rounded-full text-sm flex items-center gap-2"
                    >
                      {interest}
                      <button
                        onClick={() => toggleInterest(interest)}
                        className="hover:text-white text-orange-400/70"
                      >
                        &times;
                      </button>
                    </span>
                  ))}
                </div>
              )}

              {/* Custom interest input */}
              <div className="flex gap-2">
                <input
                  type="text"
                  value={newInterest}
                  onChange={(e) => setNewInterest(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && addInterest()}
                  placeholder="Type a topic and press Enter..."
                  className="flex-1 px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none text-sm"
                />
                <button
                  onClick={addInterest}
                  disabled={!newInterest.trim()}
                  className="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm"
                >
                  Add
                </button>
              </div>

              {/* Suggestions */}
              <div>
                <p className="text-xs text-gray-500 mb-2">Quick-add topics:</p>
                <div className="flex flex-wrap gap-2">
                  {suggestions
                    .filter(s => !interests.includes(s))
                    .slice(0, 10)
                    .map((suggestion) => (
                      <button
                        key={suggestion}
                        onClick={() => toggleInterest(suggestion)}
                        className="px-3 py-1.5 bg-bg-tertiary text-gray-400 rounded-full text-sm hover:bg-border hover:text-white transition-all"
                      >
                        + {suggestion}
                      </button>
                    ))}
                </div>
              </div>

              <p className="text-xs text-gray-500">
                4DA learns from your feedback - these are just starting points.
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Navigation */}
      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
        >
          &larr; Back
        </button>
        <button
          onClick={handleContinue}
          disabled={pullingModels}
          className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {pullingModels ? 'Installing models...' : 'Enter 4DA'}
        </button>
      </div>
    </div>
  );
}
