import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Tauri API mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

// Store captured event listeners
let ollamaListeners: Array<(event: { payload: Record<string, unknown> }) => void> = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((eventName: string, callback: (event: { payload: Record<string, unknown> }) => void) => {
    if (eventName === 'ollama-status') {
      ollamaListeners.push(callback);
    }
    return Promise.resolve(() => {
      ollamaListeners = ollamaListeners.filter((l) => l !== callback);
    });
  }),
  emit: vi.fn(),
}));

// Mock game-components (OllamaStatus uses game-status-orb)
vi.mock('../../lib/game-components', () => ({
  registerGameComponent: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { OllamaStatus } from '../OllamaStatus';
import { invoke } from '@tauri-apps/api/core';

const mockInvoke = vi.mocked(invoke);

describe('OllamaStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ollamaListeners = [];
  });

  it('renders nothing when provider is not ollama', () => {
    const { container } = render(<OllamaStatus provider="openai" />);
    expect(container.firstChild).toBeNull();
  });

  it('renders a button when provider is ollama', () => {
    render(<OllamaStatus provider="ollama" />);
    const button = screen.getByRole('button');
    expect(button).toBeInTheDocument();
  });

  it('shows offline/basic mode status by default', () => {
    render(<OllamaStatus provider="ollama" />);
    expect(screen.getByText('ollama.basicMode')).toBeInTheDocument();
  });

  it('has accessible aria-label including status', () => {
    render(<OllamaStatus provider="ollama" />);
    const button = screen.getByRole('button');
    expect(button.getAttribute('aria-label')).toContain('ollama.status');
    expect(button.getAttribute('aria-label')).toContain('ollama.basicMode');
  });

  it('button is clickable when in offline state', () => {
    render(<OllamaStatus provider="ollama" />);
    const button = screen.getByRole('button');
    expect(button).not.toBeDisabled();
  });

  it('updates status when ollama-status event is received', async () => {
    render(<OllamaStatus provider="ollama" />);

    // Simulate receiving a 'ready' event
    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'ready', model: 'llama3.2' } }),
      );
    });

    expect(screen.getByText('ollama.ready')).toBeInTheDocument();
  });

  it('shows pulling status when model is being pulled', () => {
    render(<OllamaStatus provider="ollama" />);

    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'pulling', model: 'llama3.2' } }),
      );
    });

    expect(screen.getByText('ollama.pulling')).toBeInTheDocument();
  });

  it('shows warming status when model is loading', () => {
    render(<OllamaStatus provider="ollama" />);

    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'warming', model: 'llama3.2' } }),
      );
    });

    expect(screen.getByText('ollama.loading')).toBeInTheDocument();
  });

  it('shows error status when error event received', () => {
    render(<OllamaStatus provider="ollama" />);

    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'error', model: 'llama3.2', error: 'Connection refused' } }),
      );
    });

    expect(screen.getByText('ollama.error')).toBeInTheDocument();
  });

  it('button is disabled when in ready state', () => {
    render(<OllamaStatus provider="ollama" />);

    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'ready', model: 'llama3.2' } }),
      );
    });

    const button = screen.getByRole('button');
    expect(button).toBeDisabled();
  });

  it('button is clickable when in error state', () => {
    render(<OllamaStatus provider="ollama" />);

    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'error', model: 'llama3.2', error: 'Failed' } }),
      );
    });

    const button = screen.getByRole('button');
    expect(button).not.toBeDisabled();
  });

  it('triggers retry when clicking error state button', async () => {
    render(<OllamaStatus provider="ollama" />);

    // Set to error state
    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'error', model: 'llama3.2', error: 'Failed' } }),
      );
    });

    // Click retry
    fireEvent.click(screen.getByRole('button'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('check_ollama_status', { baseUrl: null });
    });

    // Should show warming status while retrying
    expect(screen.getByText('ollama.loading')).toBeInTheDocument();
  });

  it('triggers retry when clicking offline state button', async () => {
    render(<OllamaStatus provider="ollama" />);

    // Click when in default offline state
    fireEvent.click(screen.getByRole('button'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('check_ollama_status', { baseUrl: null });
    });
  });

  it('shows error tooltip when error message is present', () => {
    render(<OllamaStatus provider="ollama" />);

    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'error', model: 'llama3.2', error: 'ECONNREFUSED' } }),
      );
    });

    const button = screen.getByRole('button');
    expect(button).toHaveAttribute('title', 'ECONNREFUSED');
  });

  it('includes click-retry hint in aria-label for clickable states', () => {
    render(<OllamaStatus provider="ollama" />);

    const button = screen.getByRole('button');
    expect(button.getAttribute('aria-label')).toContain('ollama.clickRetry');
  });

  it('does not include click-retry hint when in ready state', () => {
    render(<OllamaStatus provider="ollama" />);

    act(() => {
      ollamaListeners.forEach((cb) =>
        cb({ payload: { phase: 'ready', model: 'llama3.2' } }),
      );
    });

    const button = screen.getByRole('button');
    expect(button.getAttribute('aria-label')).not.toContain('ollama.clickRetry');
  });
});
