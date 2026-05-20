// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { listen } from '@tauri-apps/api/event';

import type { OllamaStatus, PullProgress } from './types';
import { normalizeOllamaStatus } from '../../utils/normalize-ollama';
import { fallbackSuggestions, SECTION_KEY, getPersistedSections } from './onboarding-constants';
import type { SectionState } from './onboarding-constants';
import type { ExperienceLevel } from './setup-experience';
import type { UseQuickSetupProps, ProviderType } from './quick-setup-utils';
import {
  buildInitialPullProgress, refreshOllamaAfterPull,
  validateApiKey, saveLlmProvider,
} from './quick-setup-utils';

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
  const [provider, setProvider] = useState<ProviderType>('ollama');
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
    if (status.has_embedding_model && status.has_llm_model) return;

    setPullingModels(true);
    const { models, initial } = buildInitialPullProgress(status);
    setPullProgress(initial);

    const unlisten = await listen<PullProgress>('ollama-pull-progress', (event) => {
      setPullProgress((prev) => ({ ...prev, [event.payload.model]: event.payload }));
    });

    try {
      for (const model of models) {
        setPullProgress((prev) => ({
          ...prev, [model]: { model, status: 'downloading', percent: 0, done: false },
        }));
        await cmd('pull_ollama_model', { model, baseUrl: status.base_url || null });
        setPullProgress((prev) => ({
          ...prev, [model]: { model, status: 'success', percent: 100, done: true },
        }));
      }

      const refreshed = await refreshOllamaAfterPull();
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
    void (async () => {
      try {
        const rawStatus = await cmd('check_ollama_status', { baseUrl: null }) as unknown as Record<string, unknown>;
        const status = normalizeOllamaStatus(rawStatus);
        if (cancelled) return;
        setOllamaStatus(status);

        if (status.running && status.has_embedding_model && status.has_llm_model) {
          setProvider('ollama');
          setAiConfigured(true);
          setAiOpen(false);
          setProjectsOpen(true);
        } else if (status.running) {
          setProvider('ollama');
          void pullMissingModels(status);
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
    void (async () => {
      try {
        const result = await cmd('ace_auto_discover');
        if (cancelled) return;
        setDiscoveryDone(true);
        const topics = result.scan_result?.combined?.topics || [];
        if (topics.length > 0) setDetectedTech(topics.slice(0, 12));
      } catch { setDiscoveryDone(true); }
    })();
    return () => { cancelled = true; };
  }, []);

  // --- Pre-populate from taste test if calibrated ---
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const calibrated = await cmd('taste_test_is_calibrated');
        if (cancelled || !calibrated) return;
        const profile = await cmd('taste_test_get_profile');
        if (cancelled || !profile) return;
        if (profile.topInterests.length > 0) {
          setInterests(prev => prev.length === 0 ? profile.topInterests : prev);
          setSuggestions(prev => prev.length === 0 ? profile.topInterests : prev);
        }
      } catch { /* Non-critical — taste test may not have been taken */ }
    })();
    return () => { cancelled = true; };
  }, []);

  // --- Load suggested interests after discovery ---
  useEffect(() => {
    if (!discoveryDone) return;
    let cancelled = false;
    void (async () => {
      try {
        const result = await cmd('ace_get_suggested_interests');
        if (cancelled) return;
        const topics = result.filter(s => !s.already_declared).map(s => s.topic).slice(0, 12);
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
    const timer = setTimeout(() => setProjectsOpen(true), 3000);
    return () => clearTimeout(timer);
  }, [aiConfigured]);

  const removeTag = (tag: string) => setDetectedTech(prev => prev.filter(t => t !== tag));

  const addInterest = () => {
    const trimmed = newInterest.trim();
    if (trimmed && !interests.includes(trimmed)) {
      setInterests(prev => [...prev, trimmed]);
      setNewInterest('');
    }
  };

  const toggleInterest = (topic: string) => {
    setInterests(prev =>
      prev.includes(topic) ? prev.filter(i => i !== topic) : [...prev, topic],
    );
  };

  const handleProviderChange = (p: ProviderType) => {
    setProvider(p);
    setAiConfigured(p === 'ollama' && !!ollamaStatus?.running && !!ollamaStatus.has_embedding_model && !!ollamaStatus.has_llm_model);
    if (p !== 'ollama') setProjectsOpen(true);
  };

  const handleApiKeyChange = (key: string) => {
    setApiKey(key);
    if (key.trim().length === 0) {
      setAiConfigured(false);
      setApiKeyHint(null);
      setProjectsOpen(true);
      return;
    }
    const valid = validateApiKey(provider, key);
    setAiConfigured(valid);
    setApiKeyHint(valid ? null : t('onboarding.setup.keyFormatHintSoft'));
  };

  const handleContinue = async () => {
    setError(null);
    setIsSaving(true);
    try {
      await saveLlmProvider(provider, apiKey, ollamaStatus);

      // Auto-trigger embedding engine preparation (fire-and-forget)
      // This ensures semantic search is ready by the time the user finishes onboarding
      cmd('prepare_embedding_engine').catch(() => {
        // Non-fatal: embedding engine will auto-initialize on first use
      });

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
    void handleContinue();
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
