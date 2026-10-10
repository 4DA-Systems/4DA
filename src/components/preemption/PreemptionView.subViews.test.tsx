// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// AD-054: main nav is Brief · Preemption · Signal. Preemption is the worklist,
// with Blind Spots and Knowledge Gaps folded in as supporting sub-views. These
// tests pin the shell: the sub-navigation's ARIA tabs contract, the worklist
// default, lazy sub-views, and the Signal labelling of the gated ones.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

vi.mock('./PreemptionWorklist', () => ({
  PreemptionWorklist: () => <div data-testid="worklist" />,
}));
const blindSpotsLoaded = vi.fn();
vi.mock('../blindspots/BlindSpotsView', () => {
  blindSpotsLoaded();
  return { default: () => <div data-testid="blindspots-view" /> };
});
const knowledgeLoaded = vi.fn();
vi.mock('../KnowledgeGapsPanel', () => {
  knowledgeLoaded();
  return { KnowledgeGapsPanel: () => <div data-testid="knowledge-view" /> };
});
vi.mock('../../hooks/use-telemetry', () => ({ trackEvent: vi.fn() }));

let mockState: Record<string, unknown> = {};
const setPreemptionSubView = vi.fn();
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

import PreemptionView from './PreemptionView';

function setState(overrides: Record<string, unknown> = {}) {
  mockState = {
    preemptionSubView: 'worklist',
    setPreemptionSubView,
    tier: 'free',
    trialStatus: null,
    expired: false,
    ...overrides,
  };
}

beforeEach(() => {
  setPreemptionSubView.mockClear();
  setState();
});

describe('PreemptionView — sub-views (AD-054)', () => {
  it('opens on the worklist without loading the supporting views', () => {
    render(<PreemptionView />);
    expect(screen.getByTestId('worklist')).toBeInTheDocument();
    expect(blindSpotsLoaded).not.toHaveBeenCalled();
    expect(knowledgeLoaded).not.toHaveBeenCalled();
  });

  it('is the Preemption tabpanel, titled once, with an ARIA sub-tablist', () => {
    render(<PreemptionView />);
    const panel = screen.getAllByRole('tabpanel')[0]!;
    expect(panel).toHaveAttribute('id', 'view-panel-preemption');
    expect(panel).toHaveAttribute('aria-labelledby', 'tab-preemption');
    expect(screen.getAllByRole('heading', { level: 2 })).toHaveLength(1);

    const list = screen.getByRole('tablist', { name: 'preemption.views.label' });
    const tabs = screen.getAllByRole('tab');
    expect(list).toContainElement(tabs[0]!);
    expect(tabs.map(t => t.id)).toEqual(['preemption-tab-worklist', 'preemption-tab-blindspots', 'preemption-tab-knowledge']);
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true');
    expect(tabs.map(t => t.getAttribute('tabindex'))).toEqual(['0', '-1', '-1']);

    const subPanel = screen.getAllByRole('tabpanel')[1]!;
    expect(subPanel).toHaveAttribute('id', 'preemption-panel-worklist');
    expect(subPanel).toHaveAttribute('aria-labelledby', 'preemption-tab-worklist');
    expect(tabs[0]).toHaveAttribute('aria-controls', 'preemption-panel-worklist');
  });

  it('clicking a sub-tab selects it', () => {
    render(<PreemptionView />);
    fireEvent.click(screen.getByRole('tab', { name: /preemption\.views\.blindspots/ }));
    expect(setPreemptionSubView).toHaveBeenCalledWith('blindspots');
  });

  it.each([
    ['ArrowRight', 'worklist', 'blindspots', 1],
    ['ArrowLeft', 'worklist', 'knowledge', 2],
    ['End', 'worklist', 'knowledge', 2],
    ['Home', 'knowledge', 'worklist', 0],
  ] as const)('roving tabindex: %s from %s activates and focuses %s', (key, from, to, index) => {
    setState({ preemptionSubView: from });
    render(<PreemptionView />);
    const tabs = screen.getAllByRole('tab');
    const start = tabs.find(t => t.id === `preemption-tab-${from}`)!;
    start.focus();
    fireEvent.keyDown(start, { key });
    expect(setPreemptionSubView).toHaveBeenCalledWith(to);
    expect(document.activeElement).toBe(tabs[index]);
  });

  it('renders Blind Spots lazily when selected', async () => {
    setState({ preemptionSubView: 'blindspots' });
    render(<PreemptionView />);
    expect(await screen.findByTestId('blindspots-view')).toBeInTheDocument();
    expect(screen.queryByTestId('worklist')).toBeNull();
    expect(screen.getAllByRole('tabpanel')[1]).toHaveAttribute('id', 'preemption-panel-blindspots');
  });

  it('renders Knowledge Gaps lazily when selected', async () => {
    setState({ preemptionSubView: 'knowledge' });
    await act(async () => { render(<PreemptionView />); });
    expect(await screen.findByTestId('knowledge-view')).toBeInTheDocument();
  });

  it('labels the Signal-tier sub-views for a free user — never the worklist', () => {
    render(<PreemptionView />);
    expect(screen.queryByTestId('preemption-signal-marker-worklist')).toBeNull();
    expect(screen.getByTestId('preemption-signal-marker-blindspots')).toHaveTextContent('tier.signal');
    expect(screen.getByTestId('preemption-signal-marker-knowledge')).toHaveTextContent('tier.signal');
  });

  it.each([
    ['signal tier', { tier: 'signal' }],
    ['active trial', { trialStatus: { active: true } }],
  ])('drops the Signal labels on %s', (_label, overrides) => {
    setState(overrides);
    render(<PreemptionView />);
    expect(screen.queryByTestId('preemption-signal-marker-blindspots')).toBeNull();
    expect(screen.queryByTestId('preemption-signal-marker-knowledge')).toBeNull();
  });
});
