// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';

// t() returns the key, so assertions are decoupled from locale content
// (translation parity is covered separately by validate-translations).
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('../../lib/commands', () => ({ cmd: vi.fn() }));
// Isolate the tab from the sibling sections' own data fetching + the boundary's
// i18n bootstrap (which pulls initReactI18next, absent from the partial mock above).
vi.mock('./LocaleSection', () => ({ LocaleSection: () => null }));
vi.mock('./MonitoringSection', () => ({ MonitoringSection: () => null }));
// eslint-disable-next-line @typescript-eslint/no-explicit-any
vi.mock('../PanelErrorBoundary', () => ({ PanelErrorBoundary: ({ children }: any) => children }));

import { SettingsGeneralTab } from './SettingsGeneralTab';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = (await import('../../lib/commands')) as any;

const ZERO = {
  deleted_items: 0,
  deleted_feedback: 0,
  deleted_void: 0,
  deleted_intelligence: 0,
  deleted_windows: 0,
  deleted_cycles: 0,
  deleted_necessity: 0,
  vacuumed: true,
};

function renderTab() {
  return render(
    <SettingsGeneralTab
      monitoring={null}
      monitoringInterval={30}
      setMonitoringInterval={() => {}}
      onToggleMonitoring={() => {}}
      onUpdateInterval={() => {}}
    />,
  );
}

describe('SettingsGeneralTab — maintenance button', () => {
  beforeEach(() => vi.clearAllMocks());

  it('runs deep clean and shows a records-cleaned summary', async () => {
    vi.mocked(cmd).mockImplementation((name: string) => {
      if (name === 'get_data_health') return Promise.resolve({ retention_days: 365 });
      if (name === 'run_deep_clean') {
        return Promise.resolve({ ...ZERO, deleted_items: 12, deleted_intelligence: 3 });
      }
      return Promise.resolve({});
    });
    renderTab();
    fireEvent.click(await screen.findByText('settings.dataHealth.runMaintenance'));
    await waitFor(() => expect(cmd).toHaveBeenCalledWith('run_deep_clean'));
    expect(await screen.findByText(/settings\.dataHealth\.cleanedRecords/)).toBeInTheDocument();
  });

  it('shows "already clean" when nothing was removed', async () => {
    vi.mocked(cmd).mockImplementation((name: string) => {
      if (name === 'get_data_health') return Promise.resolve({ retention_days: 30 });
      if (name === 'run_deep_clean') return Promise.resolve(ZERO);
      return Promise.resolve({});
    });
    renderTab();
    fireEvent.click(await screen.findByText('settings.dataHealth.runMaintenance'));
    await waitFor(() => expect(cmd).toHaveBeenCalledWith('run_deep_clean'));
    expect(await screen.findByText('settings.dataHealth.alreadyClean')).toBeInTheDocument();
  });

  it('does not claim a clean when the command fails', async () => {
    vi.mocked(cmd).mockImplementation((name: string) => {
      if (name === 'get_data_health') return Promise.resolve({ retention_days: 30 });
      if (name === 'run_deep_clean') return Promise.reject(new Error('boom'));
      return Promise.resolve({});
    });
    renderTab();
    fireEvent.click(await screen.findByText('settings.dataHealth.runMaintenance'));
    await waitFor(() => expect(cmd).toHaveBeenCalledWith('run_deep_clean'));
    expect(screen.queryByText('settings.dataHealth.alreadyClean')).not.toBeInTheDocument();
    expect(screen.queryByText(/settings\.dataHealth\.cleanedRecords/)).not.toBeInTheDocument();
  });
});
