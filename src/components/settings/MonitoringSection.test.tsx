// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...a) }));

import { MonitoringSection } from './MonitoringSection';
import type { MonitoringStatus } from '../../types';

const STATUS: MonitoringStatus = {
  enabled: true,
  interval_minutes: 45,
  is_checking: false,
  last_check_ago: null,
  total_checks: 3,
  close_to_tray: true,
};

function renderSection(interval: number) {
  return render(
    <MonitoringSection
      monitoring={STATUS}
      monitoringInterval={interval}
      setMonitoringInterval={() => {}}
      onToggle={() => {}}
      onUpdateInterval={() => {}}
    />,
  );
}

describe('MonitoringSection inputs', () => {
  beforeEach(() => {
    cmdMock.mockReset();
    cmdMock.mockImplementation((name: string) => {
      if (name === 'get_morning_briefing_config') return Promise.resolve({ enabled: true, time: '07:30' });
      if (name === 'get_launch_at_startup') return Promise.resolve(false);
      if (name === 'background_refresh_status') return Promise.resolve({ installed: false, supported: true, interval_minutes: null });
      return Promise.resolve({});
    });
  });

  it('shows the configured interval in a labelled field', () => {
    renderSection(45);
    const input = screen.getByRole('spinbutton', { name: 'settings.monitoring.intervalLabel' });
    expect(input).toHaveValue(45);
  });

  it('never renders a blank interval when the value is not a number', () => {
    renderSection(Number.NaN);
    expect(screen.getByRole('spinbutton', { name: 'settings.monitoring.intervalLabel' })).toHaveValue(30);
  });

  it('labels the briefing time input', async () => {
    renderSection(45);
    const time = await screen.findByLabelText('settings.monitoring.briefingTime');
    expect(time).toHaveAttribute('type', 'time');
    expect(time).toHaveValue('07:30');
  });
});
