// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tests for analysis slice error handling paths.
 *
 * Validates that startAnalysis and loadContextFiles handle IPC errors,
 * browser-mode detection, "already running" guards, and generic failures.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Must import store AFTER mocks are set up
import { useAppStore } from '../../store';

const mockedInvoke = vi.mocked(invoke);

// Capture initial state so we can reset between tests
const initialState = useAppStore.getState();

describe('startAnalysis error paths', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState(initialState);
  });

  it('sets browser mode message on IPC invoke error', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('window.invoke is not a function'));
    await useAppStore.getState().startAnalysis();

    const state = useAppStore.getState();
    expect(state.appState.loading).toBe(false);
    expect(state.appState.status).toContain('browser mode');
  });

  it('sets browser mode message on __TAURI__ error', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('__TAURI__ is not defined'));
    await useAppStore.getState().startAnalysis();

    const state = useAppStore.getState();
    expect(state.appState.loading).toBe(false);
    expect(state.appState.status).toContain('browser mode');
  });

  it('handles "already running" error without stopping loading', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('Analysis already running'));
    await useAppStore.getState().startAnalysis();

    const state = useAppStore.getState();
    expect(state.appState.status).toContain('already in progress');
  });

  it('sets generic error status on unknown failure', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('Something unexpected'));
    await useAppStore.getState().startAnalysis();

    const state = useAppStore.getState();
    expect(state.appState.loading).toBe(false);
    expect(state.appState.status).toContain('Error');
  });

  it('sets loading true while invoke is pending', async () => {
    let resolveInvoke: (v: unknown) => void;
    mockedInvoke.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveInvoke = resolve;
      }),
    );

    const promise = useAppStore.getState().startAnalysis();
    expect(useAppStore.getState().appState.loading).toBe(true);

    resolveInvoke!(undefined);
    await promise;
  });

  it('handles string rejection (non-Error throw)', async () => {
    mockedInvoke.mockRejectedValueOnce('invoke: connection refused');
    await useAppStore.getState().startAnalysis();

    const state = useAppStore.getState();
    expect(state.appState.loading).toBe(false);
    expect(state.appState.status).toContain('browser mode');
  });

  it('handles null/undefined rejection gracefully', async () => {
    mockedInvoke.mockRejectedValueOnce(null);
    await useAppStore.getState().startAnalysis();

    const state = useAppStore.getState();
    expect(state.appState.loading).toBe(false);
    expect(state.appState.status).toContain('Error');
  });
});

describe('loadContextFiles error paths', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState(initialState);
  });

  it('sets browser mode on invoke error', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('invoke is not a function'));
    await useAppStore.getState().loadContextFiles();

    const state = useAppStore.getState();
    expect(state.isBrowserMode).toBe(true);
    expect(state.appState.loading).toBe(false);
    expect(state.appState.status).toContain('Browser mode');
  });

  it('sets browser mode on __TAURI__ error', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('__TAURI__ is not defined'));
    await useAppStore.getState().loadContextFiles();

    const state = useAppStore.getState();
    expect(state.isBrowserMode).toBe(true);
    expect(state.appState.loading).toBe(false);
  });

  it('sets error status on non-IPC error', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('Database locked'));
    await useAppStore.getState().loadContextFiles();

    const state = useAppStore.getState();
    expect(state.isBrowserMode).toBeFalsy();
    expect(state.appState.loading).toBe(false);
    expect(state.appState.status).toContain('Error');
  });
});
