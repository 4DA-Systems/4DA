// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { render } from '@testing-library/react';
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

function Harness() {
  useAppListeners({
    addToast: vi.fn(),
    setEmbeddingStatus: vi.fn(),
    setShowFramework: vi.fn(),
    setShowComparison: vi.fn(),
    setState: vi.fn(),
    startAnalysis: vi.fn(),
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
});
