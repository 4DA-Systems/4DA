// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import type { InterruptionConfig, PresenceStatus } from '../../lib/commands';

// t() returns its English default with {{vars}} interpolated, so assertions
// read as the user-visible copy without coupling to locale files.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, def?: unknown, opts?: Record<string, unknown>) => {
      const template = typeof def === 'string' ? def : key;
      const vars = (typeof def === 'object' && def !== null ? def : opts) as
        | Record<string, unknown>
        | undefined;
      if (!vars) return template;
      return template.replace(/\{\{(\w+)\}\}/g, (_m, name: string) =>
        String(vars[name] ?? ''),
      );
    },
  }),
}));

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

const reportErrorMock = vi.fn();
vi.mock('../../lib/error-reporter', () => ({
  reportError: (...args: unknown[]) => reportErrorMock(...args),
}));

const { InterruptionsSection } = await import('./InterruptionsSection');

const AVAILABLE: PresenceStatus = {
  available: true,
  reason: null,
  reason_text: null,
  held_count: 0,
  os_detection_supported: true,
};

const HOLDING: PresenceStatus = {
  available: false,
  reason: 'fullscreen_app',
  reason_text: 'while you were in a fullscreen app',
  held_count: 3,
  os_detection_supported: true,
};

const DEFAULT_CONFIG: InterruptionConfig = {
  respect_focus: true,
  quiet_hours_start: null,
  quiet_hours_end: null,
  dnd_active: false,
  dnd_until: null,
};

/** Route each command to a canned response; unlisted commands resolve void. */
function wireCommands(
  config: Partial<InterruptionConfig> = {},
  status: PresenceStatus = AVAILABLE,
) {
  const merged = { ...DEFAULT_CONFIG, ...config };
  cmdMock.mockImplementation((name: string) => {
    switch (name) {
      case 'get_interruption_config':
        return Promise.resolve(merged);
      case 'get_presence_status':
        return Promise.resolve(status);
      case 'set_do_not_disturb':
        return Promise.resolve(status);
      default:
        return Promise.resolve(undefined);
    }
  });
}

/** The section renders nothing until config arrives, so wait for it. */
async function renderLoaded() {
  render(<InterruptionsSection />);
  await screen.findByText('Interruptions');
}

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  cmdMock.mockReset();
  reportErrorMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('InterruptionsSection — status', () => {
  it('reports that 4DA is ready to notify when the user is available', async () => {
    wireCommands();
    await renderLoaded();
    expect(screen.getByText('Ready to notify you')).toBeInTheDocument();
  });

  it('names the reason it is holding, in the user\'s terms', async () => {
    wireCommands({}, HOLDING);
    await renderLoaded();
    // The whole point of the feature: say WHY, in terms of what the user was
    // doing, not in terms of 4DA's internals.
    expect(
      screen.getByText('Holding while you were in a fullscreen app'),
    ).toBeInTheDocument();
  });

  it('surfaces held items with a way to see or drop them', async () => {
    wireCommands({}, HOLDING);
    await renderLoaded();
    expect(screen.getByText('3 waiting')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Show now' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Dismiss' })).toBeInTheDocument();
  });

  it('offers no held-item controls when nothing is held', async () => {
    wireCommands();
    await renderLoaded();
    expect(screen.queryByRole('button', { name: 'Show now' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Dismiss' })).toBeNull();
  });

  it('flushes held surfaces on "Show now"', async () => {
    wireCommands({}, HOLDING);
    await renderLoaded();
    fireEvent.click(screen.getByRole('button', { name: 'Show now' }));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('flush_held_notifications'),
    );
  });

  it('discards held surfaces on "Dismiss"', async () => {
    wireCommands({}, HOLDING);
    await renderLoaded();
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('discard_held_notifications'),
    );
  });
});

describe('InterruptionsSection — respect focus', () => {
  const NAME = 'Respect fullscreen and focus';

  it('is on by default', async () => {
    wireCommands();
    await renderLoaded();
    expect(screen.getByRole('switch', { name: NAME })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });

  it('persists when turned off', async () => {
    wireCommands();
    await renderLoaded();
    fireEvent.click(screen.getByRole('switch', { name: NAME }));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_respect_focus', { enabled: false }),
    );
  });

  it('reverts the optimistic flip when persistence fails', async () => {
    wireCommands();
    await renderLoaded();
    cmdMock.mockImplementationOnce(() => Promise.reject(new Error('ipc down')));
    fireEvent.click(screen.getByRole('switch', { name: NAME }));
    await waitFor(() =>
      expect(screen.getByRole('switch', { name: NAME })).toHaveAttribute(
        'aria-checked',
        'true',
      ),
    );
    expect(reportErrorMock).toHaveBeenCalled();
  });

  it('says so when the platform has no fullscreen detection', async () => {
    wireCommands({}, { ...AVAILABLE, os_detection_supported: false });
    await renderLoaded();
    // Honesty rule: never imply a capability the platform does not have.
    expect(
      screen.getByText(/Fullscreen detection is not available on this platform/),
    ).toBeInTheDocument();
  });
});

describe('InterruptionsSection — Do Not Disturb', () => {
  const NAME = 'Do Not Disturb';

  it('turns on indefinitely from the toggle', async () => {
    wireCommands();
    await renderLoaded();
    fireEvent.click(screen.getByRole('switch', { name: NAME }));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_do_not_disturb', {
        enabled: true,
        minutes: null,
      }),
    );
  });

  it('offers timed durations while DND is off', async () => {
    wireCommands();
    await renderLoaded();
    expect(screen.getByRole('button', { name: '30m' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '1h' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '2h' })).toBeInTheDocument();
  });

  it('sends the chosen duration in minutes', async () => {
    wireCommands();
    await renderLoaded();
    fireEvent.click(screen.getByRole('button', { name: '2h' }));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_do_not_disturb', {
        enabled: true,
        minutes: 120,
      }),
    );
  });

  it('hides the duration chips once DND is on', async () => {
    wireCommands({ dnd_active: true });
    await renderLoaded();
    expect(screen.queryByRole('button', { name: '30m' })).toBeNull();
  });

  it('turns DND off when already on', async () => {
    wireCommands({ dnd_active: true });
    await renderLoaded();
    fireEvent.click(screen.getByRole('switch', { name: NAME }));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_do_not_disturb', {
        enabled: false,
        minutes: null,
      }),
    );
  });
});

describe('InterruptionsSection — quiet hours', () => {
  const NAME = 'Quiet hours';

  it('is off when either end is unset', async () => {
    wireCommands({ quiet_hours_start: '22:00', quiet_hours_end: null });
    await renderLoaded();
    expect(screen.getByRole('switch', { name: NAME })).toHaveAttribute(
      'aria-checked',
      'false',
    );
  });

  it('seeds a sensible overnight window when switched on', async () => {
    wireCommands();
    await renderLoaded();
    fireEvent.click(screen.getByRole('switch', { name: NAME }));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_quiet_hours', {
        start: '22:00',
        end: '07:00',
      }),
    );
  });

  it('clears both ends when switched off', async () => {
    wireCommands({ quiet_hours_start: '22:00', quiet_hours_end: '07:00' });
    await renderLoaded();
    fireEvent.click(screen.getByRole('switch', { name: NAME }));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_quiet_hours', {
        start: null,
        end: null,
      }),
    );
  });

  it('shows both time inputs when the window is set', async () => {
    wireCommands({ quiet_hours_start: '22:00', quiet_hours_end: '07:00' });
    await renderLoaded();
    const times = screen.getAllByDisplayValue(/^(22:00|07:00)$/);
    expect(times).toHaveLength(2);
  });

  it('reverts and reports when the backend rejects the window', async () => {
    wireCommands({ quiet_hours_start: '22:00', quiet_hours_end: '07:00' });
    await renderLoaded();
    cmdMock.mockImplementationOnce(() => Promise.reject(new Error('bad HH:MM')));
    fireEvent.click(screen.getByRole('switch', { name: NAME }));
    await waitFor(() => expect(reportErrorMock).toHaveBeenCalled());
    // Still on, because the clear never took effect.
    expect(screen.getByRole('switch', { name: NAME })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });
});

describe('InterruptionsSection — resilience', () => {
  it('renders nothing rather than a broken panel when config cannot load', async () => {
    cmdMock.mockImplementation((name: string) =>
      name === 'get_interruption_config'
        ? Promise.reject(new Error('no backend'))
        : Promise.resolve(AVAILABLE),
    );
    const { container } = render(<InterruptionsSection />);
    await waitFor(() => expect(reportErrorMock).toHaveBeenCalled());
    expect(container.querySelector('input[type="time"]')).toBeNull();
    expect(screen.queryByText('Interruptions')).toBeNull();
  });

  it('reports a failed status poll without tearing down the panel', async () => {
    cmdMock.mockImplementation((name: string) =>
      name === 'get_presence_status'
        ? Promise.reject(new Error('poll failed'))
        : Promise.resolve(DEFAULT_CONFIG),
    );
    await renderLoaded();
    await waitFor(() =>
      expect(reportErrorMock).toHaveBeenCalledWith(
        'InterruptionsSection.get_presence_status',
        expect.anything(),
      ),
    );
    // The panel itself is still up — the status line is advisory only.
    expect(screen.getByText('Interruptions')).toBeInTheDocument();
  });
});
