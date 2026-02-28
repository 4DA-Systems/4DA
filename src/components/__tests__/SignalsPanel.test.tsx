import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Tauri API mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { SignalsPanel } from '../SignalsPanel';
import { makeItem } from '../../test/factories';

function makeSignalItem(overrides = {}) {
  return makeItem({
    signal_type: 'security_alert',
    signal_priority: 'high',
    signal_action: 'Update dependency immediately',
    signal_triggers: ['CVE-2025-001'],
    ...overrides,
  });
}

describe('SignalsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders without crash with empty results', () => {
    render(<SignalsPanel results={[]} />);
    expect(screen.getByText('signals.noSignals')).toBeInTheDocument();
  });

  it('shows empty state when no results have signal fields', () => {
    // Items without signal_type/signal_priority/signal_action are filtered out
    render(<SignalsPanel results={[makeItem()]} />);
    expect(screen.getByText('signals.noSignals')).toBeInTheDocument();
  });

  it('renders signal items when results have signal data', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Patch this vulnerability' }),
        ]}
      />,
    );
    expect(screen.getByText('Patch this vulnerability')).toBeInTheDocument();
  });

  it('shows the signals title header', () => {
    render(
      <SignalsPanel results={[makeSignalItem({ id: 1 })]} />,
    );
    expect(screen.getByText('signals.title')).toBeInTheDocument();
  });

  it('shows signal count in subtitle', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1 }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'medium', signal_action: 'Monitor' }),
        ]}
      />,
    );
    expect(screen.getByText('signals.actionable')).toBeInTheDocument();
  });

  it('displays critical count badge when critical signals exist', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_priority: 'critical', signal_action: 'Emergency patch' }),
        ]}
      />,
    );
    expect(screen.getByText('signals.critical')).toBeInTheDocument();
  });

  it('displays high count badge when high priority signals exist', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_priority: 'high', signal_action: 'Update soon' }),
        ]}
      />,
    );
    expect(screen.getByText('signals.high')).toBeInTheDocument();
  });

  it('sorts signals by priority (critical first)', () => {
    const { container } = render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_priority: 'low', signal_action: 'Low priority item' }),
          makeSignalItem({ id: 2, signal_priority: 'critical', signal_action: 'Critical item' }),
          makeSignalItem({ id: 3, signal_priority: 'high', signal_action: 'High priority item' }),
        ]}
      />,
    );
    // Get all signal action texts in order
    const actions = container.querySelectorAll('.text-sm.font-medium');
    const texts = Array.from(actions).map((el) => el.textContent);
    expect(texts[0]).toBe('Critical item');
    expect(texts[1]).toBe('High priority item');
    expect(texts[2]).toBe('Low priority item');
  });

  it('collapses panel when header is clicked', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Visible action' }),
        ]}
      />,
    );

    // Initially expanded
    expect(screen.getByText('Visible action')).toBeInTheDocument();

    // Click header to collapse
    fireEvent.click(screen.getByLabelText('signals.title'));

    // Signal content should be hidden
    expect(screen.queryByText('Visible action')).not.toBeInTheDocument();
  });

  it('re-expands panel when header is clicked again', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Toggle action' }),
        ]}
      />,
    );

    // Collapse
    fireEvent.click(screen.getByLabelText('signals.title'));
    expect(screen.queryByText('Toggle action')).not.toBeInTheDocument();

    // Expand
    fireEvent.click(screen.getByLabelText('signals.title'));
    expect(screen.getByText('Toggle action')).toBeInTheDocument();
  });

  it('shows type filter buttons for each signal type', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'medium', signal_action: 'Watch trend' }),
        ]}
      />,
    );

    // "Security" appears in both filter and signal row badge, so use getAllByText
    expect(screen.getAllByText('Security').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Trends').length).toBeGreaterThanOrEqual(1);
  });

  it('filters by type when type filter button is clicked', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert', signal_action: 'Patch vuln' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'medium', signal_action: 'Watch trend' }),
        ]}
      />,
    );

    // Click the Security filter button (it has a count child element).
    // The filter buttons are in the filter bar; find the first "Security" that is inside a button.
    const securityElements = screen.getAllByText('Security');
    const filterBtn = securityElements.find((el) => el.closest('button[class*="rounded-lg"]'))?.closest('button');
    expect(filterBtn).toBeTruthy();
    fireEvent.click(filterBtn!);

    // Only security items should be visible
    expect(screen.getByText('Patch vuln')).toBeInTheDocument();
    expect(screen.queryByText('Watch trend')).not.toBeInTheDocument();
  });

  it('clears type filter when clicking active filter button', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert', signal_action: 'Patch vuln' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'medium', signal_action: 'Watch trend' }),
        ]}
      />,
    );

    // Find and click the Security filter button
    const getFilterBtn = () => {
      const els = screen.getAllByText('Security');
      return els.find((el) => el.closest('button[class*="rounded-lg"]'))?.closest('button');
    };

    // Activate filter
    fireEvent.click(getFilterBtn()!);
    expect(screen.queryByText('Watch trend')).not.toBeInTheDocument();

    // Deactivate filter
    fireEvent.click(getFilterBtn()!);
    expect(screen.getByText('Watch trend')).toBeInTheDocument();
  });

  it('shows "clear" button when filters are active', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert', signal_action: 'Patch vuln' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'medium', signal_action: 'Trend' }),
        ]}
      />,
    );

    // No clear button initially
    expect(screen.queryByText('signals.clear')).not.toBeInTheDocument();

    // Find and click the Security filter button
    const securityElements = screen.getAllByText('Security');
    const filterBtn = securityElements.find((el) => el.closest('button[class*="rounded-lg"]'))?.closest('button');
    fireEvent.click(filterBtn!);

    // Clear button should appear
    expect(screen.getByText('signals.clear')).toBeInTheDocument();
  });

  it('clears all filters when clear button is clicked', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert', signal_action: 'Patch vuln' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'medium', signal_action: 'Trend signal' }),
        ]}
      />,
    );

    // Find and click the Security filter button
    const securityElements = screen.getAllByText('Security');
    const filterBtn = securityElements.find((el) => el.closest('button[class*="rounded-lg"]'))?.closest('button');
    fireEvent.click(filterBtn!);
    expect(screen.queryByText('Trend signal')).not.toBeInTheDocument();

    // Click clear
    fireEvent.click(screen.getByText('signals.clear'));

    // All items should be visible again
    expect(screen.getByText('Patch vuln')).toBeInTheDocument();
    expect(screen.getByText('Trend signal')).toBeInTheDocument();
  });

  it('shows trigger toggle button when signal has triggers', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            signal_triggers: ['CVE-2025-001', 'dependency-update'],
          }),
        ]}
      />,
    );

    expect(screen.getByText('signals.triggers')).toBeInTheDocument();
  });

  it('shows similar items count when signal has similar items', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            similar_count: 3,
            similar_titles: ['Similar Item A', 'Similar Item B'],
          }),
        ]}
      />,
    );

    expect(screen.getByText(/signals\.similar/)).toBeInTheDocument();
  });
});
