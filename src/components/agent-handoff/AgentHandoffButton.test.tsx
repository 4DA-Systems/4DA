// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AgentHandoffButton, resetAgentHandoffCache } from './AgentHandoffButton';
import { cmd } from '../../lib/commands';
import type { AgentHandoffTargets } from '../../../src-tauri/bindings/bindings/AgentHandoffTargets';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(),
}));

const mockCmd = vi.mocked(cmd);
const writeText = vi.fn((_text: string) => Promise.resolve());

const item = {
  id: 'preempt-1',
  kind: 'alert',
  title: 'lodash advisory',
  explanation: '',
  urgency: 'high',
  confidence: { value: 0.9, provenance: 'osv_verified', sample_size: null },
  reversibility: null,
  evidence: [{ source: 'version_context', title: 'Installed: 4.17.20 \u2192 update to >= 4.17.21 (direct)', url: null, freshness_days: 0, relevance_note: '' }],
  affected_deps: ['lodash'],
  affected_projects: ['D:\\code\\shop'],
  suggested_actions: [],
  precedents: [],
  refutation_condition: null,
  lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: false, no_coverage: false },
  created_at: BigInt(0),
  expires_at: null,
} as unknown as EvidenceItem;

function withTargets(targets: Partial<AgentHandoffTargets>, outcome: 'prefilled' | 'paste' = 'prefilled') {
  const full: AgentHandoffTargets = {
    claude_code: 'missing', claude_code_route: 'prefill', vscode_claude: 'missing', cursor: 'missing', ...targets,
  };
  mockCmd.mockImplementation(((name: string) =>
    Promise.resolve(name === 'agent_handoff_targets' ? full : outcome)));
}

describe('AgentHandoffButton', () => {
  beforeEach(() => {
    resetAgentHandoffCache();
    mockCmd.mockReset();
    writeText.mockClear();
    Object.assign(navigator, { clipboard: { writeText } });
  });

  it('offers only "Copy prompt" when no agent can be opened', async () => {
    withTargets({});
    render(<AgentHandoffButton mode="fix" item={item} />);
    const copy = await screen.findByText('agentHandoff.copy');
    fireEvent.click(copy);
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    expect(writeText.mock.calls[0]?.[0]).toContain('Package: lodash');
    expect(mockCmd).not.toHaveBeenCalledWith('open_agent_handoff', expect.anything());
  });

  it('opens Claude Code with the fix prompt in the project folder', async () => {
    withTargets({ claude_code: 'ready' });
    render(<AgentHandoffButton mode="fix" item={item} />);
    fireEvent.click(await screen.findByText('agentHandoff.fix.claude_code'));
    await waitFor(() => expect(mockCmd).toHaveBeenCalledWith('open_agent_handoff', expect.objectContaining({
      target: 'claude_code',
      cwd: 'D:\\code\\shop',
    })));
    // Prefill route: nothing is copied.
    expect(writeText).not.toHaveBeenCalled();
  });

  it('copies first, then opens Claude Code, and explains the paste on the paste route', async () => {
    withTargets({ claude_code: 'ready', claude_code_route: 'paste' }, 'paste');
    render(<AgentHandoffButton mode="fix" item={item} />);
    fireEvent.click(await screen.findByText('agentHandoff.fix.claude_code'));
    expect(await screen.findByText('agentHandoff.pasteHint')).toBeDefined();
    expect(writeText).toHaveBeenCalledTimes(1);
    const copyOrder = writeText.mock.invocationCallOrder[0] ?? -1;
    const openCall = mockCmd.mock.calls.findIndex(c => c[0] === 'open_agent_handoff');
    const openOrder = mockCmd.mock.invocationCallOrder[openCall] ?? -1;
    expect(copyOrder).toBeGreaterThan(0);
    expect(openOrder).toBeGreaterThan(copyOrder);
  });

  it('does not open Claude Code on the paste route when the copy fails', async () => {
    withTargets({ claude_code: 'ready', claude_code_route: 'paste' }, 'paste');
    writeText.mockImplementationOnce(() => Promise.reject(new Error('no focus')));
    render(<AgentHandoffButton mode="brief" />);
    fireEvent.click(await screen.findByText('agentHandoff.brief.claude_code'));
    expect(await screen.findByText('agentHandoff.copyFailed')).toBeDefined();
    expect(mockCmd).not.toHaveBeenCalledWith('open_agent_handoff', expect.anything());
  });

  it('puts the other ready agents and "Copy prompt" in the menu, without a cwd for VS Code', async () => {
    withTargets({ claude_code: 'ready', vscode_claude: 'ready', cursor: 'ready' });
    render(<AgentHandoffButton mode="brief" />);
    fireEvent.click(await screen.findByLabelText('agentHandoff.more'));
    expect(screen.getByText('agentHandoff.brief.cursor')).toBeDefined();
    expect(screen.getByText('agentHandoff.copy')).toBeDefined();
    fireEvent.click(screen.getByText('agentHandoff.brief.vscode_claude'));
    await waitFor(() => expect(mockCmd).toHaveBeenCalledWith('open_agent_handoff', expect.objectContaining({
      target: 'vscode_claude',
      cwd: null,
    })));
  });

  it('renders the menu outside the card (so overflow cannot clip it) and closes it on Escape or an outside press', async () => {
    withTargets({ claude_code: 'ready', vscode_claude: 'ready' });
    const { container } = render(<AgentHandoffButton mode="brief" />);
    const more = await screen.findByLabelText('agentHandoff.more');
    fireEvent.click(more);
    const menu = screen.getByRole('menu');
    expect(container.contains(menu)).toBe(false);
    expect(menu.style.position).toBe('fixed');
    expect(more.getAttribute('aria-expanded')).toBe('true');
    fireEvent.mouseDown(menu);
    expect(screen.queryByRole('menu')).not.toBeNull();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
    fireEvent.click(more);
    expect(screen.queryByRole('menu')).not.toBeNull();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('shows the first-run hint when Claude Code has no link handler yet', async () => {
    withTargets({ claude_code: 'needs_first_run' });
    render(<AgentHandoffButton mode="brief" />);
    expect(await screen.findByText('agentHandoff.needsFirstRun')).toBeDefined();
    expect(screen.getByText('agentHandoff.copy')).toBeDefined();
  });

  it('shows the backend reason when opening fails', async () => {
    mockCmd.mockImplementation(((name: string) => name === 'agent_handoff_targets'
      ? Promise.resolve({ claude_code: 'ready', claude_code_route: 'prefill', vscode_claude: 'missing', cursor: 'missing' })
      : Promise.reject({ code: 'E6001', title: 'Invalid input', detail: 'Cannot open the agent there: the folder does not exist.' })));
    render(<AgentHandoffButton mode="fix" item={item} />);
    fireEvent.click(await screen.findByText('agentHandoff.fix.claude_code'));
    expect(await screen.findByText('Cannot open the agent there: the folder does not exist.')).toBeDefined();
  });

  it('builds the prompt from the full finding, not the trimmed list copy', async () => {
    withTargets({ claude_code: 'ready' });
    const trimmed = { ...item, evidence: [] } as EvidenceItem;
    const resolveItem = vi.fn(() => Promise.resolve(item));
    render(<AgentHandoffButton mode="fix" item={trimmed} resolveItem={resolveItem} />);
    fireEvent.click(await screen.findByText('agentHandoff.fix.claude_code'));
    await waitFor(() => expect(mockCmd).toHaveBeenCalledWith('open_agent_handoff', expect.objectContaining({
      prompt: expect.stringContaining('Installed: 4.17.20 (direct)') as unknown as string,
    })));
    expect(resolveItem).toHaveBeenCalledTimes(1);
  });

  it('falls back to the list copy when the full finding cannot be fetched', async () => {
    withTargets({});
    const resolveItem = vi.fn(() => Promise.reject(new Error('gone')));
    render(<AgentHandoffButton mode="fix" item={item} resolveItem={resolveItem} />);
    fireEvent.click(await screen.findByText('agentHandoff.copy'));
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    expect(writeText.mock.calls[0]?.[0]).toContain('Package: lodash');
  });

  it('renders nothing for a finding with no dependency to fix', () => {
    withTargets({ claude_code: 'ready' });
    const { container } = render(<AgentHandoffButton mode="fix" item={{ ...item, affected_deps: [] }} />);
    expect(container.innerHTML).toBe('');
  });
});
