// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// AI-provider step: a failed key import is reported, a picked local server
// says what will be used (no key box), and a failed or cancellable download
// shows its real state.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

import { SetupAIProvider } from './setup-ai-provider';

const base = {
  ollamaStatus: null,
  provider: 'anthropic' as const,
  apiKey: '',
  pullingModels: false,
  pullProgress: {},
  onProviderChange: vi.fn(),
  onApiKeyChange: vi.fn(),
};

function backend(overrides: Record<string, () => unknown> = {}) {
  cmdMock.mockImplementation((command: string) => {
    const override = overrides[command];
    if (override) return override();
    if (command === 'detect_environment') {
      return Promise.resolve({
        has_anthropic_env: true, anthropic_env_preview: 'sk-ant-...abcd',
        has_openai_env: false, openai_env_preview: '', ollama_running: false, ollama_url: null,
      });
    }
    if (command === 'detect_local_servers') return Promise.resolve({ servers: [] });
    return Promise.resolve();
  });
}

describe('SetupAIProvider', () => {
  beforeEach(() => { cmdMock.mockReset(); });

  it('reports a failed environment-key import instead of failing silently', async () => {
    backend({ import_env_key: () => Promise.reject('keychain unavailable') });
    const onApiKeyChange = vi.fn();
    render(<SetupAIProvider {...base} onApiKeyChange={onApiKeyChange} />);
    await act(async () => {});

    await act(async () => {
      fireEvent.click(screen.getByText('onboarding.setupAi.useThisKey'));
    });

    expect(screen.getByRole('alert')).toHaveTextContent('onboarding.setupAi.importFailed');
    expect(onApiKeyChange).not.toHaveBeenCalled();
  });

  it('labels the API key input', async () => {
    backend();
    render(<SetupAIProvider {...base} />);
    await act(async () => {});
    expect(screen.getByLabelText(/settings\.llm\.apiKey/)).toHaveAttribute('type', 'password');
  });

  it('a picked local server shows what will be used, with no key box or cloud notice', async () => {
    backend();
    render(
      <SetupAIProvider
        {...base}
        provider="openai-compatible"
        localServer={{ name: 'LM Studio', baseUrl: 'http://localhost:1234/v1', model: 'qwen3-14b' }}
      />,
    );
    await act(async () => {});

    expect(screen.getByRole('status')).toHaveTextContent('onboarding.setupAi.localServerSelected');
    expect(screen.queryByText('onboarding.setupAi.otherProviderHint')).not.toBeInTheDocument();
    expect(screen.queryByText(/onboarding\.setupAi\.cloudDataDisclosure|What gets sent/)).not.toBeInTheDocument();
  });

  it('shows a failed model pull as failed and disables Cancel while cancelling', async () => {
    backend();
    render(
      <SetupAIProvider
        {...base}
        pullingModels
        cancellingDownload
        onCancelDownload={vi.fn()}
        pullProgress={{ 'llama3.2': { model: 'llama3.2', status: 'failed', percent: 0, done: false } }}
      />,
    );
    await act(async () => {});

    expect(screen.getByText('onboarding.setupAi.pullFailedShort')).toBeInTheDocument();
    expect(screen.getByText('onboarding.setupAi.cancellingDownload')).toBeDisabled();
    expect(screen.getByText('onboarding.apiKeys.pullSizeMessage')).toBeInTheDocument();
  });
});
