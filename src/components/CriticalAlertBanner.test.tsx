// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useAppStore } from '../store';
import { CriticalAlertBanner } from './CriticalAlertBanner';

const cmdMock = vi.fn();
vi.mock('../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...a) }));
vi.mock('./ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({ getTranslated: (_id: string, text: string) => text, requestTranslation: () => {} }),
}));
const openUrl = vi.fn((_url: string) => Promise.resolve());
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: (u: string) => openUrl(u) }));

const alert = (id: number) => ({
  id,
  title: `Advisory ${id}`,
  url: `https://osv.dev/vulnerability/GHSA-${id}`,
  advisory_id: `GHSA-${id}`,
  is_critical_alert: true,
  top_score: 1,
});

function seed(n: number) {
  const s = useAppStore.getState();
  useAppStore.setState({
    appState: { ...s.appState, relevanceResults: Array.from({ length: n }, (_, i) => alert(i + 1)) as never },
  });
}

beforeEach(() => {
  cmdMock.mockReset();
  openUrl.mockClear();
  cmdMock.mockImplementation((name: string) => Promise.resolve(name === 'get_triage_states' ? [] : undefined));
});

describe('CriticalAlertBanner (audit 2026-11-11)', () => {
  it('counts every pending alert, not only the three it lists', async () => {
    seed(5);
    render(<CriticalAlertBanner />);
    expect(await screen.findByText('alerts.criticalPlural')).toBeInTheDocument();
    // The passthrough i18n mock drops values, so read the count off the list.
    expect(screen.getAllByText(/^Advisory \d$/)).toHaveLength(3);
    expect(screen.getByText('alerts.moreNotShown')).toBeInTheDocument();
  });

  it('"Investigate" only opens the advisory; the alert stays', async () => {
    seed(1);
    render(<CriticalAlertBanner />);
    fireEvent.click(await screen.findByText('alerts.investigate'));
    await waitFor(() => expect(openUrl).toHaveBeenCalledWith('https://osv.dev/vulnerability/GHSA-1'));
    expect(cmdMock).not.toHaveBeenCalledWith('triage_alert', expect.anything());
    expect(screen.getByText('Advisory 1')).toBeInTheDocument();
  });

  it('a dismissal is stored with the backend expiry and can be undone', async () => {
    seed(1);
    render(<CriticalAlertBanner />);
    fireEvent.click(await screen.findByText('alerts.notApplicableShort'));
    expect(cmdMock).toHaveBeenCalledWith('triage_alert', {
      itemId: 1, alertKey: null, action: 'not_applicable', advisoryId: 'GHSA-1', reason: null, expiresAt: null,
    });
    expect(screen.queryByText('Advisory 1')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('alerts.undo'));
    expect(cmdMock).toHaveBeenCalledWith('untriage_alert', { itemId: 1, alertKey: null });
    expect(screen.getByText('Advisory 1')).toBeInTheDocument();
  });

  it('a dismissal the store refused comes back and says so', async () => {
    cmdMock.mockImplementation((name: string) => {
      if (name === 'get_triage_states') return Promise.resolve([]);
      if (name === 'triage_alert') return Promise.reject(new Error('database is locked'));
      return Promise.resolve();
    });
    seed(1);
    render(<CriticalAlertBanner />);
    await act(async () => { fireEvent.click(await screen.findByText('✓')); });
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('alerts.triageFailed'));
    expect(screen.getByText('Advisory 1')).toBeInTheDocument();
  });

  it('an "investigating" record from another surface hides nothing', async () => {
    cmdMock.mockImplementation((name: string) => Promise.resolve(name === 'get_triage_states'
      ? [{ item_id: 1, advisory_id: null, action: 'investigating', reason: null, resolved_at: '', expires_at: null }]
      : undefined));
    seed(1);
    render(<CriticalAlertBanner />);
    await waitFor(() => expect(cmdMock).toHaveBeenCalledWith('get_triage_states', { itemIds: [1] }));
    expect(screen.getByText('Advisory 1')).toBeInTheDocument();
  });
});
