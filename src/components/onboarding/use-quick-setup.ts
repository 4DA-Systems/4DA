// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { listen } from '@tauri-apps/api/event';

import type { OllamaStatus, PullProgress } from './types';
import { fallbackSuggestions, SECTION_KEY, getPersistedSections } from './onboarding-constants';
import type { SectionState } from './onboarding-constants';
import type { ExperienceLevel } from './setup-experience';

interface UseQuickSetupProps {
  isAnimating: boolean;
  onComplete: () => void;
  onBack: () => void;
}

export function useQuickSetup({ onComplete }: UseQuickSetupProps) {
  const { t } = useTranslation();

  // Section collapse state — restore from localStorage if available
  const persisted = getPersistedSections();
  const [aiOpen, setAiOpen] = useState(persisted.aiOpen ?? true);
  const [projectsOpen, setProjectsOpen] = useState(persisted.projectsOpen ?? false);
  const [stacksOpen, setStacksOpen] = useState(persisted.stacksOpen ?? false);
  const [interestsOpen, setInterestsOpen] = useState(persisted.interestsOpen ?? false);
  const [localeOpen, setLocaleOpen] = useState(persisted.localeOpen ?? false);
  const [experienceOpen, setExperienceOpen] = useState(persisted.experienceOpen ?? false);
  const [localeConfigured, setLocaleConfigured] = useState(false);
  const [selectedStacks, setSelectedStacks] = useState<string[]>([]);
  const [experienceLevel, setExperienceLevel] = useState<ExperienceLevel | null>(null);

  // AI Provider state
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus | null>(null);
  const [provider, setProvider] = useState<'anthropic' | 'openai' | 'ollama' | 'openai-compatible'>('ollama');
  const [apiKey, setApiKey] = useState('');
  const [pullingModels, setPullingModels] = useState(false);
  const [pullProgress, setPullProgress] = useState<Record<string, PullProgress>>({});
  const [aiConfigured, setAiConfigured] = useState(false);

  // Projects + Interests state
  const [detectedTech, setDetectedTech] = useState<string[]>([]);
  const [discoveryDone, setDiscoveryDone] = useState(false);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [interests, setInterests] = useState<string[]>([]);
  const [newInterest, setNewInterest] = useState('');
  const [role, setRole] = useState('Developer');
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [apiKeyHint, setApiKeyHint] = useState<string | null>(null);
  const [skippedDownload, setSkippedDownload] = useState(false);

  // Persist section state to localStorage
  useEffect(() => {
    try {
      const state: SectionState = { aiOpen, projectsOpen, stacksOpen, interestsOpen, localeOpen, experienceOpen };
      localStorage.setItem(SECTION_KEY, JSON.stringify(state));
    } catch { /* noop */ }
  }, [aiOpen, projectsOpen, stacksOpen, interestsOpen, localeOpen, experienceOpen]);

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
        await cmd('pull_ollama_model', {
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
          const s = await cmd('check_ollama_status', { baseUrl: null }) as unknown as OllamaStatus;
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
        const status = await cmd('check_ollama_status', { baseUrl: null }) as unknown as OllamaStatus;
        if (cancelled) return;
        setOllamaStatus(status);

        if (status.running && status.has_embedding_model && status.has_llm_model) {
          setProvider('ollama');
          setAiConfigured(true);
          setAiOpen(false);
          setProjectsOpen(true);
        } else if (status.running) {
          setProvider('ollama');
          pullMissingModels(status);
        }
      } catch {
        setOllamaStatus({ running: false, version: null, models: [], base_url: 'http://localhost:11434' } as OllamaStatus);
      }
    })();
    return () => { cancelled = true; };
  }, [pullMissingModels]);

  // --- Auto-discover projects ---
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await cmd('ace_auto_discover');
        if (cancelled) return;
        setDiscoveryDone(true);

        const topics = result.scan_result?.combined?.topics || [];
        if (topics.length > 0) {
          setDetectedTech(topics.slice(0, 12));
        }
      } catch {
        setDiscoveryDone(true);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // --- Pre-populate from taste test if calibrated ---
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const calibrated = await cmd('taste_test_is_calibrated');
        if (cancelled || !calibrated) return;
        const profile = await cmd('taste_test_get_profile');
        if (cancelled || !profile) return;
        if (profile.topInterests.length > 0) {
          setInterests(prev => prev.length === 0 ? profile.topInterests : prev);
          setSuggestions(prev => prev.length === 0 ? profile.topInterests : prev);
        }
      } catch {
        // Non-critical — taste test may not have been taken
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
        const result = await cmd('ace_get_suggested_interests');
        if (cancelled) return;
        const topics = result
          .filter(s => !s.already_declared)
          .map(s => s.topic)
          .slice(0, 12);
        const finalSuggestions = topics.length > 0 ? topics : fallbackSuggestions;
        setSuggestions(finalSuggestions);
        setInterests(prev => prev.length === 0 ? finalSuggestions.slice(0, 5) : prev);
      } catch {
        setSuggestions(fallbackSuggestions);
        setInterests(prev => prev.length === 0 ? fallbackSuggestions.slice(0, 5) : prev);
      }
    })();
    return () => { cancelled = true; };
  }, [discoveryDone]);

  // Auto-expand next section on completion
  useEffect(() => { if (aiConfigured) setProjectsOpen(true); }, [aiConfigured]);
  useEffect(() => { if (discoveryDone) setStacksOpen(true); }, [discoveryDone]);
  useEffect(() => { if (selectedStacks.length > 0) setLocaleOpen(true); }, [selectedStacks.length]);
  useEffect(() => { if (localeConfigured) setInterestsOpen(true); }, [localeConfigured]);
  useEffect(() => { if (interests.length > 0) setExperienceOpen(true); }, [interests.length]);

  // Auto-expand remaining sections after a delay if AI not configured
  useEffect(() => {
    if (aiConfigured) return;
    const timer = setTimeout(() => {
      setProjectsOpen(true);
    }, 3000);
    return () => clearTimeout(timer);
  }, [aiConfigured]);

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

  const handleProviderChange = (p: 'anthropic' | 'openai' | 'ollama' | 'openai-compatible') => {
    setProvider(p);
    setAiConfigured(p === 'ollama' && !!ollamaStatus?.running && !!ollamaStatus.has_embedding_model && !!ollamaStatus.has_llm_model);
    if (p !== 'ollama') {
      setProjectsOpen(true);
    }
  };

  const handleApiKeyChange = (key: string) => {
    setApiKey(key);
    if (key.trim().length === 0) {
      setAiConfigured(false);
      setApiKeyHint(null);
      setProjectsOpen(true);
      return;
    }
    let valid = false;
    if (provider === 'anthropic') {
      valid = key.startsWith('sk-ant-') && key.length > 20;
    } else if (provider === 'openai') {
      valid = key.startsWith('sk-') && key.length > 20;
    } else if (provider === 'openai-compatible') {
      valid = key.trim().length > 10; // Unknown format — just check non-empty
    } else {
      valid = key.trim().length > 10;
    }
    setAiConfigured(valid);
    setApiKeyHint(valid ? null : t('onboarding.setup.keyFormatHintSoft'));
  };

  const saveLlmProvider = async () => {
    const noProvider = { provider: 'none', apiKey: '', model: '', baseUrl: null, openaiApiKey: null };
    if (provider === 'ollama') {
      if (ollamaStatus?.running) {
        const ollamaModel = ollamaStatus.models?.find(m => !m.startsWith('nomic-embed-text')) || 'llama3.2';
        await cmd('set_llm_provider', {
          provider: 'ollama', apiKey: '', model: ollamaModel,
          baseUrl: ollamaStatus.base_url || 'http://localhost:11434', openaiApiKey: null,
        });
      } else {
        await cmd('set_llm_provider', noProvider);
      }
    } else if (provider === 'openai-compatible' && apiKey.trim()) {
      // OpenAI-compatible: save key now, user configures base_url + model in Settings
      await cmd('set_llm_provider', {
        provider: 'openai-compatible', apiKey, model: '',
        baseUrl: null, openaiApiKey: null,
      });
    } else if (apiKey.trim()) {
      const model = provider === 'anthropic' ? 'claude-haiku-4-5-20251001' : 'gpt-4o-mini';
      await cmd('set_llm_provider', {
        provider, apiKey, model, baseUrl: null,
        openaiApiKey: provider === 'openai' ? apiKey : null,
      });
    } else {
      await cmd('set_llm_provider', noProvider);
    }
  };

  const handleContinue = async () => {
    setError(null);
    setIsSaving(true);
    try {
      await saveLlmProvider();
      if (role) await cmd('set_user_role', { role });
      if (experienceLevel) await cmd('set_experience_level', { level: experienceLevel });

      const interestsToSave = interests.length > 0
        ? interests
        : detectedTech.length > 0 ? detectedTech.slice(0, 5) : fallbackSuggestions.slice(0, 3);
      await Promise.all([
        ...interestsToSave.map(interest => cmd('add_interest', { topic: interest })),
        ...detectedTech.map(tech => cmd('add_tech_stack', { technology: tech })),
        ...(selectedStacks.length > 0 ? [cmd('set_selected_stacks', { profileIds: selectedStacks })] : []),
      ]);

      try { localStorage.removeItem(SECTION_KEY); } catch { /* noop */ }
      onComplete();
    } catch (e) {
      setError(`Failed to save settings: ${e}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleSkipDownload = () => {
    setPullingModels(false);
    setSkippedDownload(true);
    setTimeout(() => setSkippedDownload(false), 3000);
    handleContinue();
  };

  return {
    t,
    aiOpen, setAiOpen, projectsOpen, setProjectsOpen,
    stacksOpen, setStacksOpen, interestsOpen, setInterestsOpen,
    localeOpen, setLocaleOpen, localeConfigured, setLocaleConfigured,
    experienceOpen, setExperienceOpen, experienceLevel, setExperienceLevel,
    selectedStacks, setSelectedStacks,
    ollamaStatus, provider, apiKey, pullingModels, pullProgress, aiConfigured,
    detectedTech, discoveryDone,
    suggestions, interests, newInterest, setNewInterest, role, setRole,
    error, setError, isSaving, apiKeyHint, skippedDownload,
    removeTag, addInterest, toggleInterest,
    handleProviderChange, handleApiKeyChange, handleContinue, handleSkipDownload,
  };
}
