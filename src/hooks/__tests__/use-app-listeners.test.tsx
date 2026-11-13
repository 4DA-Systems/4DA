// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { act, render, waitFor } from '@testing-library/react';
import { describe, expect, it, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

import { useAppStore } from '../../store';
import { useAppListeners } from '../use-app-listeners';
import { isVictauriDogfoodMode } from '../../lib/startup-runtime';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('../../lib/startup-runtime', () => ({
  isVictauriDogfoodMode: vi.fn(() => Promise.resolve(false)),
}));

const initialState = useAppStore.getState();

function Harness({ startAnalysis = vi.fn() }: { startAnalysis?: () => void }) {
  useAppListeners({
    addToast: vi.fn(),
    setEmbeddingStatus: vi.fn(),
    setShowFramework: vi.fn(),
    setShowComparison: vi.fn(),
    setState: vi.fn(),
    startAnalysis,
  });
  return null;
}

describe('useAppListeners', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState(initialState, true);
  });

  it('does not probe startup analysis IPC in browser mode', async () => {
    useAppStore.setState({ isBrowserMode: true });

    render(<Harness />);
    await Promise.resolve();

    expect(isVictauriDogfoodMode).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
  });

  describe('mount-time analysis (audit 2026-11-11: network before consent)', () => {
    beforeEach(() => {
      window.sessionStorage.clear();
      vi.mocked(invoke).mockImplementation((command: string) =>
        Promise.resolve(command === 'get_analysis_status' ? { results: [] } : null));
    });

    it('waits for the onboarding decision, and starts nothing when the wizard opens', async () => {
      useAppStore.setState({ settingsLoaded: false, onboardingKnown: false, showOnboarding: false, isFirstRun: false });
      const startAnalysis = vi.fn();
      render(<Harness startAnalysis={startAnalysis} />);
      await waitFor(() => expect(invoke).toHaveBeenCalledWith('get_analysis_status', expect.anything()));
      await act(async () => { await new Promise(r => setTimeout(r, 20)); });
      expect(startAnalysis).not.toHaveBeenCalled();
      act(() => { useAppStore.setState({ showOnboarding: true, settingsLoaded: true, onboardingKnown: true }); });
      await act(async () => { await new Promise(r => setTimeout(r, 20)); });
      expect(startAnalysis).not.toHaveBeenCalled();
    });

    // PR #1131 review: a get_settings that misses the 5 s bootstrap timeout
    // lifts the splash with showOnboarding still false. That is not
    // "onboarding complete"; the wizard may open once settings arrive.
    it('a timed-out settings load is not an onboarding decision', async () => {
      useAppStore.setState({ settingsLoaded: true, onboardingKnown: false, showOnboarding: false, isFirstRun: false });
      const startAnalysis = vi.fn();
      render(<Harness startAnalysis={startAnalysis} />);
      await waitFor(() => expect(invoke).toHaveBeenCalledWith('get_analysis_status', expect.anything()));
      await act(async () => { await new Promise(r => setTimeout(r, 20)); });
      expect(startAnalysis).not.toHaveBeenCalled();
      act(() => { useAppStore.setState({ showOnboarding: true, onboardingKnown: true }); });
      await act(async () => { await new Promise(r => setTimeout(r, 20)); });
      expect(startAnalysis).not.toHaveBeenCalled();
    });

    it('starts one analysis once the user is in the app', async () => {
      useAppStore.setState({ settingsLoaded: true, onboardingKnown: true, showOnboarding: false, isFirstRun: false });
      const startAnalysis = vi.fn();
      render(<Harness startAnalysis={startAnalysis} />);
      await waitFor(() => expect(startAnalysis).toHaveBeenCalledTimes(1));
    });
  });
});
