// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { SettingsIntelligenceTab } from './SettingsIntelligenceTab';

// Each section is stubbed so the test pins only the tab's own wiring. Your
// Stack throws, as it did when its IPC answered with a non-array: before it had
// its own PanelErrorBoundary, that throw escaped to the modal-level boundary
// and replaced the whole Settings dialog, Save button included.
vi.mock('./YourStackSection', () => ({
  YourStackSection: () => {
    throw new Error('stack section exploded');
  },
}));
vi.mock('./AIProviderSection', () => ({ AIProviderSection: () => <div>ai-provider</div> }));
vi.mock('./BlindSpotsAssessSection', () => ({ BlindSpotsAssessSection: () => null }));
vi.mock('./StandingQueriesSection', () => ({ StandingQueriesSection: () => null }));
vi.mock('./LicenseSection', () => ({ LicenseSection: () => null }));

describe('SettingsIntelligenceTab section isolation', () => {
  it('a crashing Your Stack section stays contained; the rest of the tab still renders', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SettingsIntelligenceTab
        settings={null}
        settingsForm={{} as never}
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
    expect(screen.getByText('ai-provider')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'settings.ai.saveConfiguration' })).toBeInTheDocument();
  });
});
