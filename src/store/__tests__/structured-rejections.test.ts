// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Store slices against rejections shaped as the backend really sends them.
//
// A command returning `FourDaError` rejects with an object
// `{ code, title, detail, remediation, severity }` (src-tauri/src/error.rs).
// Every slice test before this one rejected with `new Error(...)`, which is
// why `String(e)` and `${error}` passed review and "[object Object]" reached
// Settings, onboarding and the team panel.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

import { useAppStore } from '../index';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const initialState = useAppStore.getState();

/** `FourDaError::Internal(msg)`: the catch-all arm of `to_user_error`. */
function internal(detail: string) {
  return {
    code: 'E9999',
    title: 'Unexpected error',
    detail,
    remediation: ['Try the operation again.', 'If this persists, restart 4DA.'],
    severity: 'error',
  };
}

const DB_LOCKED = {
  code: 'E3001',
  title: 'Database busy',
  detail: 'The database is temporarily locked by another operation.',
  remediation: ['Wait a moment and try again.', 'If this persists, restart 4DA.'],
  severity: 'warning',
};

describe('store slices show a structured rejection as text', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  it('user context: the settings status carries the backend sentence', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(internal('Interest already exists'));
    useAppStore.getState().setNewInterest('rust');

    await useAppStore.getState().addInterest();

    expect(useAppStore.getState().settingsStatus).toBe('Error: Interest already exists');
  });

  it('scan directories: the settings status carries the backend sentence', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(internal('Path is outside your home directory'));
    useAppStore.getState().setNewScanDir('C:/elsewhere');

    await useAppStore.getState().addScanDirectory();

    expect(useAppStore.getState().settingsStatus).toBe('Error: Path is outside your home directory');
  });

  it('system health: a specific code reads as its category', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(DB_LOCKED);

    await useAppStore.getState().resolveAnomaly(7);

    expect(useAppStore.getState().settingsStatus).toBe('Error: Database error. Try restarting the app.');
  });

  it('channels: the render error is the backend sentence', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(internal('Channel not found'));

    await useAppStore.getState().selectChannel(999);

    expect(useAppStore.getState().renderError).toBe('Channel not found');
  });

  it('channels: a spent AI budget in a structured rejection is localized, token removed', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(
      internal('Daily AI budget reached — resets at 00:00 local time (in 2 h 5 min) [budget_reset:00:00:125].'),
    );

    await useAppStore.getState().renderChannel(1);

    expect(useAppStore.getState().renderError).toBe('Daily AI budget reached — resets at 00:00 (in 125 minutes)');
  });

  it('team, decisions and trust: the stored error is readable', async () => {
    vi.mocked(invoke).mockRejectedValue(internal('Team relay is not configured'));

    await useAppStore.getState().loadTeamStatus();
    await useAppStore.getState().loadDecisions();
    await useAppStore.getState().loadTrustSummary();

    expect(useAppStore.getState().teamError).toBe('Team relay is not configured');
    expect(useAppStore.getState().decisionsError).toBe('Team relay is not configured');
    expect(useAppStore.getState().trustError).toBe('Team relay is not configured');
  });

  it('analysis: "already running" is still recognised inside a structured rejection', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(internal('Analysis already running'));

    await useAppStore.getState().startAnalysis();

    // Classified as in-progress, not reported as a failed analysis.
    expect(useAppStore.getState().appState.status).toBe('Analysis already in progress...');
    expect(useAppStore.getState().toasts.some((toast) => toast.type === 'error')).toBe(false);
  });

  it('no slice stores "[object Object]"', async () => {
    vi.mocked(invoke).mockRejectedValue(internal('x'));
    const s = useAppStore.getState();
    s.setNewInterest('a');
    s.setNewExclusion('b');
    s.setNewTechStack('c');

    await s.addInterest();
    const afterInterest = useAppStore.getState().settingsStatus;
    await s.addExclusion();
    const afterExclusion = useAppStore.getState().settingsStatus;
    await s.addTechStack();
    const afterTech = useAppStore.getState().settingsStatus;
    await s.loadChannels();
    await s.runAnomalyDetection();

    const state = useAppStore.getState();
    expect([afterInterest, afterExclusion, afterTech, state.settingsStatus]).toEqual([
      'Error: x', 'Error: x', 'Error: x', 'Error: x',
    ]);
    expect(state.channelsError).toBe('x');
  });
});
