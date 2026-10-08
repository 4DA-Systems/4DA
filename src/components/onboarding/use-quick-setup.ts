// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { runDiscovery } from '../../lib/discovery';
import { safeListen } from '../../lib/tauri-events';

import type { OllamaStatus, PullProgress } from './types';
import { normalizeOllamaStatus } from '../../utils/normalize-ollama';
import { fallbackSuggestions, SECTION_KEY, getPersistedSections } from './onboarding-constants';
import type { SectionState } from './onboarding-constants';
import type { ExperienceLevel } from './setup-experience';
import type { UseQuickSetupProps, ProviderType, LocalServerChoice } from './quick-setup-utils';
import {
  buildInitialPullProgress, refreshOllamaAfterPull, isOllamaReady,
  validateApiKey, saveLlmProvider, probeKeyBeforeSave,
} from './quick-setup-utils';
import { useDiscoveryFolders } from './use-discovery-folders';

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
  // No provider until the user picks one (or a ready local Ollama is shown
  // to them as the selection): Enter never saves a provider they did not see.
  const [provider, setProvider] = useState<ProviderType | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [pullingModels, setPullingModels] = useState(false);
  const [pullProgress, setPullProgress] = useState<Record<string, PullProgress>>({});
  const [aiConfigured, setAiConfigured] = useState(false);
  const [localServer, setLocalServer] = useState<LocalServerChoice | null>(null);

  // Projects + Interests state
  const discovery = useDiscoveryFolders();
  const [detectedTech, setDetectedTech] = useState<string[]>([]);
  const [scanning, setScanning] = useState(false);
  const [discoveryDone, setDiscoveryDone] = useState(false);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [interests, setInterests] = useState<string[]>([]);
  const [newInterest, setNewInterest] = useState('');
  const [role, setRole] = useState('Developer');
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [apiKeyHint, setApiKeyHint] = useState<string | null>(null);
  const [skippedDownload, setSkippedDownload] = useState(false);
  const [cancellingDownload, setCancellingDownload] = useState(false);
  // Set while the user's own Cancel is in flight, so the pull's resulting
  // "cancelled" rejection is not reported as a download failure.
  const cancelRequested = useRef(false);

  // Persist section state to localStorage
  useEffect(() => {
    try {
      const state: SectionState = { aiOpen, projectsOpen, stacksOpen, interestsOpen, localeOpen, experienceOpen };
      localStorage.setItem(SECTION_KEY, JSON.stringify(state));
    } catch { /* noop */ }
  }, [aiOpen, projectsOpen, stacksOpen, interestsOpen, localeOpen, experienceOpen]);

  // --- AI Provider auto-detect ---
  const pullMissingModels = useCallback(async (status: OllamaStatus) => {
    if (status.has_llm_model) return;

    setPullingModels(true);
    cancelRequested.current = false;
    const { models, initial } = buildInitialPullProgress(status);
    setPullProgress(initial);

    let unlisten: (() => void) | null = null;
    try {
      // Inside the try: if subscribing throws, the finally below still clears
      // pullingModels — otherwise the provider picker stays hidden for good.
      unlisten = await safeListen<PullProgress>('ollama-pull-progress', (event) => {
        setPullProgress((prev) => ({ ...prev, [event.payload.model]: event.payload }));
      });
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
        setAiConfigured(isOllamaReady(refreshed));
      }
    } catch (e) {
      if (!cancelRequested.current) {
        setError(t('onboarding.setup.downloadFailed', { error: String(e) }));
      }
    } finally {
      unlisten?.();
      cancelRequested.current = false;
      setCancellingDownload(false);
      setPullingModels(false);
    }
  }, [t]);

  const cancelDownload = useCallback(async () => {
    if (cancellingDownload) return;
    cancelRequested.current = true;
    setCancellingDownload(true);
    try {
      await cmd('cancel_ollama_pull');
    } catch (e) {
      cancelRequested.current = false;
      setCancellingDownload(false);
      setError(t('onboarding.setup.cancelDownloadFailed', { error: String(e) }));
    }
  }, [cancellingDownload, t]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const rawStatus = await cmd('check_ollama_status', { baseUrl: null }) as unknown as Record<string, unknown>;
        const status = normalizeOllamaStatus(rawStatus);
        if (cancelled) return;
        setOllamaStatus(status);

        // A running Ollama with a chat model is ready: it is shown selected,
        // and the user can pick another provider. Without a chat model nothing
        // is selected and nothing is pulled — the download is the user's call.
        if (isOllamaReady(status)) {
          setProvider('ollama');
          setAiConfigured(true);
          setAiOpen(false);
          setProjectsOpen(true);
        }
      } catch {
        setOllamaStatus({ running: false, version: null, models: [], base_url: 'http://localhost:11434' });
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // --- Project scan: only the folders the user ticked, only when they ask ---
  const scanProjects = useCallback(async () => {
    if (scanning || discovery.selected.length === 0) return;
    setScanning(true);
    try {
      const result = await runDiscovery(discovery.selected);
      const topics = result.scan_result?.combined?.topics || [];
      if (topics.length > 0) setDetectedTech(topics.slice(0, 12));
      setDiscoveryDone(true);
    } catch (e) {
      setError(t('onboarding.projects.scanFailed', { error: String(e) }));
    } finally {
      setScanning(false);
    }
  }, [scanning, discovery.selected, t]);

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

  // --- Load suggested interests (again after a scan) ---
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const result = await cmd('ace_get_suggested_interests');
        if (cancelled) return;
        const topics = result.filter(s => !s.already_declared).map(s => s.topic).slice(0, 12);
        // Only PRE-SELECT interests derived from REAL detection. The generic
        // fallback list (ML/Rust/TS...) is shown as clickable suggestions but
        // never auto-committed: silently seeding 3+ phantom interests on an
        // empty machine defeats the thin-profile scoring floor and hands a
        // C#/PHP dev a confident Rust feed they never chose (F-6). With no
        // detection, interests start empty and the conservative floor engages.
        const finalSuggestions = topics.length > 0 ? topics : fallbackSuggestions;
        setSuggestions(finalSuggestions);
        if (topics.length > 0) {
          setInterests(prev => prev.length === 0 ? topics.slice(0, 5) : prev);
        }
      } catch {
        // Detection failed — offer suggestions, but do not auto-commit them.
        setSuggestions(fallbackSuggestions);
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

  // Explicit, user-initiated local-model download. Replaces the old silent
  // auto-pull on mount — models only download when the user asks.
  const downloadLocalModels = useCallback(() => {
    if (ollamaStatus?.running) void pullMissingModels(ollamaStatus);
  }, [ollamaStatus, pullMissingModels]);

  // A detected local server (LM Studio, llama.cpp, Jan) is a complete choice on
  // its own: base URL + loaded model, no key needed.
  const handleLocalServerSelect = (choice: LocalServerChoice) => {
    setProvider('openai-compatible');
    setLocalServer(choice);
    setApiKey('');
    setApiKeyHint(null);
    setAiConfigured(true);
    setProjectsOpen(true);
  };

  const handleProviderChange = (p: ProviderType) => {
    setProvider(p);
    setLocalServer(null);
    setAiConfigured(p === 'ollama' && isOllamaReady(ollamaStatus));
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
    const valid = provider !== null && validateApiKey(provider, key);
    setAiConfigured(valid);
    setApiKeyHint(valid ? null : t('onboarding.setup.keyFormatHintSoft'));
  };

  const handleContinue = async () => {
    setError(null);
    setIsSaving(true);
    try {
      // Pre-flight: live-probe a keyed cloud provider and block ONLY on a
      // definitive rejection (bad format or 401/403). Network blips pass.
      const probe = await probeKeyBeforeSave(provider, apiKey);
      if (!probe.ok) {
        setApiKeyHint(probe.reason ?? null);
        setAiConfigured(false);
        setIsSaving(false);
        return;
      }

      await saveLlmProvider(provider, apiKey, ollamaStatus, localServer);

      // Auto-trigger embedding engine preparation (fire-and-forget)
      // This ensures semantic search is ready by the time the user finishes onboarding
      cmd('prepare_embedding_engine').catch(() => {
        // Non-fatal: embedding engine will auto-initialize on first use
      });

      if (role) await cmd('set_user_role', { role });
      if (experienceLevel) await cmd('set_experience_level', { level: experienceLevel });

      // Save the user's chosen interests, or fall back to REAL detected tech.
      // Never persist the generic fallback list — an empty interest set is the
      // honest thin-profile state the scoring floor is built for (F-6).
      const interestsToSave = interests.length > 0
        ? interests
        : detectedTech.slice(0, 5);
      await Promise.all([
        ...interestsToSave.map(interest => cmd('add_interest', { topic: interest })),
        ...detectedTech.map(tech => cmd('add_tech_stack', { technology: tech })),
        ...(selectedStacks.length > 0 ? [cmd('set_selected_stacks', { profileIds: selectedStacks })] : []),
      ]);

      try { localStorage.removeItem(SECTION_KEY); } catch { /* noop */ }
      onComplete();
    } catch (e) {
      setError(t('onboarding.setup.saveFailed', { error: String(e) }));
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
    ollamaStatus, provider, apiKey, pullingModels, pullProgress, aiConfigured, localServer,
    discovery, scanning, scanProjects, detectedTech, discoveryDone,
    suggestions, interests, newInterest, setNewInterest, role, setRole,
    error, setError, isSaving, apiKeyHint, skippedDownload, cancellingDownload,
    removeTag, addInterest, toggleInterest,
    handleProviderChange, handleLocalServerSelect, handleApiKeyChange, handleContinue, handleSkipDownload,
    downloadLocalModels, cancelDownload,
  };
}
