// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Signal Lane 1, the stack-change stream: loading, error + retry, cold start
 * (no lockfile read: onboarding, doctrine rule 6), "nothing changed", the
 * free-floor note, the 20-row cap, the command copy and axe.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { axe, toHaveNoViolations } from 'jest-axe';
import type { EvidenceFeed } from '../../../src-tauri/bindings/bindings/EvidenceFeed';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';

expect.extend(toHaveNoViolations);

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({ cmd: (...args: unknown[]) => cmdMock(...args) }));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts && 'count' in opts ? `${key}:${String(opts.count)}` : key,
    i18n: { language: 'en' },
  }),
}));
const storeState = {
  isScanning: false,
  runAutoDiscovery: vi.fn(() => Promise.resolve()),
  loadUserContext: vi.fn(() => Promise.resolve()),
};
vi.mock('../../store', () => ({
  useAppStore: (selector: (s: typeof storeState) => unknown) => selector(storeState),
}));
vi.mock('../ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({ getTranslated: (_: string, t: string) => t, requestTranslation: vi.fn() }),
}));

import { StackChangeLane } from './StackChangeLane';
import { STACK_LANE_CAP } from './stack-change';

function item(id: string, title: string, extra: Partial<EvidenceItem> = {}): EvidenceItem {
  return {
    id,
    kind: 'alert',
    title,
    explanation: 'Version-confirmed, because the installed copy is inside the affected range.',
    confidence: { value: 0.95, provenance: 'osv_verified' },
    urgency: 'high',
    evidence: [
      { source: 'version_context', title: 'crates.io · rmcp 1.7.0 → 1.7.1', freshness_days: 0, relevance_note: 'security' },
      { source: 'osv', title: 'GHSA-x', url: 'https://osv.dev/vulnerability/GHSA-x', freshness_days: 1 },
    ],
    affected_projects: ['D:/work/4da/src-tauri'],
    affected_deps: ['rmcp'],
    suggested_actions: [
      { action_id: 'review_security', label: 'Read the advisory', description: 'Open GHSA-x on osv.dev' },
      { action_id: 'run_command', label: 'cargo update -p rmcp@1.7.0 --precise 1.7.1', description: 'Run in 4da/src-tauri' },
    ],
    lens_hints: {},
    created_at: 0,
    ...extra,
  } as EvidenceItem;
}

function feed(items: EvidenceItem[], extra: Partial<EvidenceFeed> = {}): EvidenceFeed {
  return {
    items, total: items.length, critical_count: 0, high_count: items.length, score: null,
    total_tracked: 120, tier_scope: 'full', ...extra,
  } as EvidenceFeed;
}

describe('StackChangeLane', () => {
  beforeEach(() => {
    cmdMock.mockReset();
    storeState.runAutoDiscovery.mockClear();
  });

  it('asks the backend for the stream and renders each change with its badge, versions and command', async () => {
    cmdMock.mockResolvedValue(feed([item('stack-change:security:crates.io:rmcp:4da/src-tauri', 'rmcp 1.7.0 → 1.7.1: security fix (high)')]));
    render(<StackChangeLane />);
    expect(screen.getByText('signals.stack.loading')).toBeInTheDocument();
    expect(await screen.findByText('rmcp 1.7.0 → 1.7.1: security fix (high)')).toBeInTheDocument();
    expect(cmdMock).toHaveBeenCalledWith('get_stack_changes', { force: false });
    expect(screen.getByText('signals.stack.change.security')).toBeInTheDocument();
    expect(screen.getByText('rmcp 1.7.0 → 1.7.1')).toBeInTheDocument();
    expect(screen.getByText('crates.io')).toBeInTheDocument();
    expect(screen.getByText('cargo update -p rmcp@1.7.0 --precise 1.7.1')).toBeInTheDocument();
    expect(screen.getByText('4da/src-tauri')).toBeInTheDocument();
  });

  it('copies the exact command', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText } });
    cmdMock.mockResolvedValue(feed([item('stack-change:minor:crates.io:serde@1.2.0', 'serde 1.0.0 → 1.2.0: new minor')]));
    render(<StackChangeLane />);
    fireEvent.click(await screen.findByRole('button', { name: 'signals.stack.copyAria' }));
    expect(writeText).toHaveBeenCalledWith('cargo update -p rmcp@1.7.0 --precise 1.7.1');
    expect(await screen.findByText('signals.stack.copied')).toBeInTheDocument();
  });

  it('shows an error with a retry that forces a recompute', async () => {
    cmdMock.mockRejectedValueOnce(new Error('boom')).mockResolvedValueOnce(feed([]));
    render(<StackChangeLane />);
    expect(await screen.findByRole('alert')).toHaveTextContent('signals.stack.error');
    fireEvent.click(screen.getByRole('button', { name: 'signals.stack.retry' }));
    expect(await screen.findByTestId('stack-lane-empty')).toBeInTheDocument();
    expect(cmdMock).toHaveBeenLastCalledWith('get_stack_changes', { force: true });
  });

  it('is onboarding, not an empty card, when no lockfile has been read (rule 6)', async () => {
    cmdMock.mockResolvedValueOnce(feed([], { total_tracked: 0 })).mockResolvedValueOnce(feed([]));
    render(<StackChangeLane />);
    const cold = await screen.findByTestId('stack-lane-cold-start');
    expect(within(cold).getByText('signals.stack.coldTitle')).toBeInTheDocument();
    expect(screen.queryByText('signals.stack.emptyTitle')).toBeNull();
    fireEvent.click(within(cold).getByRole('button', { name: 'onboarding.choice.scanProjects' }));
    await waitFor(() => expect(storeState.runAutoDiscovery).toHaveBeenCalled());
    await waitFor(() => expect(cmdMock).toHaveBeenLastCalledWith('get_stack_changes', { force: true }));
  });

  it('says "Nothing changed in your stack" when lockfiles were read and nothing changed', async () => {
    cmdMock.mockResolvedValue(feed([]));
    render(<StackChangeLane />);
    const empty = await screen.findByTestId('stack-lane-empty');
    expect(empty).toHaveTextContent('signals.stack.emptyTitle');
    expect(empty).toHaveTextContent('signals.stack.emptyBody');
  });

  it('names what the free floor leaves out instead of implying nothing was released', async () => {
    cmdMock.mockResolvedValue(feed([], { tier_scope: 'free_floor' }));
    render(<StackChangeLane />);
    expect(await screen.findByTestId('stack-lane-empty')).toHaveTextContent('signals.stack.emptyBodyFree');
    expect(screen.getByText('signals.stack.releasesSignal')).toBeInTheDocument();
  });

  it(`caps the lane at ${STACK_LANE_CAP} behind an explicit "Show all N"`, async () => {
    const many = Array.from({ length: STACK_LANE_CAP + 5 }, (_, i) => item(`stack-change:minor:npm:p${i}@1.1.0`, `p${i} 1.0.0 → 1.1.0: new minor`));
    cmdMock.mockResolvedValue(feed(many));
    render(<StackChangeLane />);
    await screen.findByText('p0 1.0.0 → 1.1.0: new minor');
    expect(screen.getAllByRole('article')).toHaveLength(STACK_LANE_CAP);
    const btn = screen.getByRole('button', { name: `signals.laneShowAll:${STACK_LANE_CAP + 5}` });
    expect(btn).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(btn);
    expect(screen.getAllByRole('article')).toHaveLength(STACK_LANE_CAP + 5);
  });

  it('has no axe violations with items rendered', async () => {
    cmdMock.mockResolvedValue(feed([
      item('stack-change:security:crates.io:rmcp:a', 'rmcp 1.7.0 → 1.7.1: security fix (high)'),
      item('stack-change:major:crates.io:tokio@2.0.0', 'tokio 1.40.0 → 2.0.0: major release', { suggested_actions: [{ action_id: 'check_breaking', label: 'Read the release notes', description: '' }] }),
    ]));
    const { container } = render(<StackChangeLane />);
    await screen.findByText('tokio 1.40.0 → 2.0.0: major release');
    expect(await axe(container)).toHaveNoViolations();
  });
});
