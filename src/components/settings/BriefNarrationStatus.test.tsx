// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve({})) }));
// t() returns the key, so we assert the component picks the correct key per reason —
// decoupled from locale content (translation parity is covered by validate-translations).
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('../../lib/commands', () => ({ cmd: vi.fn() }));

import { BriefNarrationStatus } from './BriefNarrationStatus';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = (await import('../../lib/commands')) as any;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function settingsWith(provider: string, model: string, hasKey: boolean): any {
  return { llm: { provider, model, has_api_key: hasKey, base_url: null } };
}

describe('BriefNarrationStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders nothing until capability resolves', () => {
    vi.mocked(cmd).mockReturnValue(new Promise(() => {})); // never resolves
    const { container } = render(<BriefNarrationStatus settings={settingsWith('none', '', false)} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when the command fails', async () => {
    vi.mocked(cmd).mockRejectedValue(new Error('boom'));
    const { container } = render(<BriefNarrationStatus settings={settingsWith('none', '', false)} />);
    await waitFor(() => expect(cmd).toHaveBeenCalledWith('get_brief_capability'));
    expect(container.firstChild).toBeNull();
  });

  it('shows the AI-narrated copy when the model is capable', async () => {
    vi.mocked(cmd).mockResolvedValue({
      brief_capable: true,
      reason: 'capable',
      provider: 'anthropic',
      model: 'claude-sonnet-4-6',
    });
    render(<BriefNarrationStatus settings={settingsWith('anthropic', 'claude-sonnet-4-6', true)} />);
    expect(await screen.findByText('settings.ai.briefStatusTitle')).toBeInTheDocument();
    expect(screen.getByText('settings.ai.briefNarrated')).toBeInTheDocument();
  });

  it('shows the no-LLM floor copy when no model is configured', async () => {
    vi.mocked(cmd).mockResolvedValue({
      brief_capable: false,
      reason: 'no_llm',
      provider: 'none',
      model: '',
    });
    render(<BriefNarrationStatus settings={settingsWith('none', '', false)} />);
    expect(await screen.findByText('settings.ai.briefFloorNoLlm')).toBeInTheDocument();
    expect(screen.queryByText('settings.ai.briefNarrated')).not.toBeInTheDocument();
  });

  it('shows the too-weak floor copy for a weak model', async () => {
    vi.mocked(cmd).mockResolvedValue({
      brief_capable: false,
      reason: 'model_too_weak',
      provider: 'anthropic',
      model: 'claude-haiku-4-5',
    });
    render(<BriefNarrationStatus settings={settingsWith('anthropic', 'claude-haiku-4-5', true)} />);
    expect(await screen.findByText('settings.ai.briefFloorWeak')).toBeInTheDocument();
    expect(screen.queryByText('settings.ai.briefFloorNoLlm')).not.toBeInTheDocument();
  });
});
