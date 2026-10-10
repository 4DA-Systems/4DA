// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

import { AIProviderSection } from './AIProviderSection';
import { ReRankingSection } from './ReRankingSection';
import { SettingsIntelligenceTab } from './SettingsIntelligenceTab';
import type { SettingsForm } from './ai-provider-types';

// Fresh-profile E2E 2026-10-10, profile A ("Skip — no AI for now"): Settings
// showed the provider as Anthropic / claude-sonnet-5, LLM re-ranking checked,
// and the Intelligence Engine named "Arctic Embed M" while nomic was running.

const mockCmd = vi.fn();
vi.mock('../../lib/commands', () => ({ cmd: (...args: unknown[]) => mockCmd(...args) }));
vi.mock('./APIKeyInput', () => ({ APIKeyInput: () => <div data-testid="api-key-input" /> }));
vi.mock('./BriefNarrationStatus', () => ({ BriefNarrationStatus: () => null }));
vi.mock('./ModelEvalSection', () => ({ ModelEvalSection: () => null }));
vi.mock('./UsageStatsSection', () => ({ UsageStatsSection: () => null }));
vi.mock('../calibration/CalibrationSettingsRow', () => ({ CalibrationSettingsRow: () => null }));
vi.mock('./YourStackSection', () => ({ YourStackSection: () => null }));
vi.mock('./BlindSpotsAssessSection', () => ({ BlindSpotsAssessSection: () => null }));
vi.mock('./StandingQueriesSection', () => ({ StandingQueriesSection: () => null }));
vi.mock('./LicenseSection', () => ({ LicenseSection: () => null }));

const noneForm: SettingsForm = {
  provider: 'none',
  apiKey: '',
  model: '',
  baseUrl: '',
  rerankEnabled: true, // the saved flag A had: inert without a provider
  maxItems: 15,
  minScore: 0.25,
  dailyTokenLimit: 100000,
  dailyCostLimit: 50,
};

beforeEach(() => {
  mockCmd.mockReset();
  mockCmd.mockImplementation((name: string) =>
    name === 'get_embedding_model_info'
      ? Promise.resolve({ model: 'nomic-embed-text', reembed_in_progress: false, multilingual_model: 'x', engine: 'nomic-embed-text-v1.5' })
      : Promise.resolve(null));
});

describe('AI provider "none" is shown as none', () => {
  it('selects None and shows no key or model for a provider that does not exist', () => {
    render(
      <AIProviderSection
        settings={null}
        settingsForm={noneForm}
        setSettingsForm={vi.fn()}
        ollamaStatus={null}
        ollamaModels={[]}
        checkOllamaStatus={vi.fn()}
        modelRegistry={null}
        onRefreshRegistry={vi.fn()}
      />,
    );
    expect(screen.getByLabelText('settings.ai.provider')).toHaveValue('none');
    expect(screen.getByTestId('ai-provider-none-hint')).toHaveTextContent('settings.ai.providerNoneHint');
    expect(screen.queryByTestId('api-key-input')).toBeNull();
    expect(screen.queryByLabelText('settings.ai.model')).toBeNull();
    expect(screen.queryByText('claude-sonnet-5')).toBeNull();
  });

  it('re-ranking reads off, and cannot be ticked, without a provider', () => {
    render(<ReRankingSection settingsForm={noneForm} setSettingsForm={vi.fn()} />);
    const box = screen.getByRole('checkbox');
    expect(box).not.toBeChecked();
    expect(box).toBeDisabled();
    expect(screen.getByText('settings.ai.rerankNeedsProvider')).toBeInTheDocument();
  });

  it('re-ranking keeps its saved state once a provider exists', () => {
    render(<ReRankingSection settingsForm={{ ...noneForm, provider: 'ollama' }} setSettingsForm={vi.fn()} />);
    const box = screen.getByRole('checkbox');
    expect(box).toBeChecked();
    expect(box).toBeEnabled();
  });
});

describe('Intelligence Engine names the embedding model in use', () => {
  it('shows the engine the backend reports, not a hard-coded name', async () => {
    render(
      <SettingsIntelligenceTab
        settings={null}
        settingsForm={noneForm}
        setSettingsForm={vi.fn()}
        ollamaStatus={null}
        ollamaModels={[]}
        checkOllamaStatus={vi.fn()}
        modelRegistry={null}
        onRefreshRegistry={vi.fn()}
        setSettingsStatus={vi.fn()}
        saveSettings={vi.fn()}
        testConnection={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId('engine-embedding-model')).toHaveTextContent('nomic-embed-text-v1.5');
    });
    expect(screen.queryByText(/Arctic/)).toBeNull();
  });
});
