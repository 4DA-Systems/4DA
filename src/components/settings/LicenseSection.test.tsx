// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Settings > License. A refused activation used to end as the raw key
// "error.license.validationFailed" wrapped around "Unknown error": the key
// lives in the `errors` namespace and was looked up in `ui`, and the backend's
// structured rejection was flattened before it got here.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

interface MockTrialStatus {
  active: boolean;
  days_remaining: number;
  started_at: string | null;
  has_license: boolean;
}

const activateLicense = vi.fn();
const startTrial = vi.fn();
let state: Record<string, unknown>;

vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) => selector(state),
    { getState: () => state },
  ),
}));

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(() => Promise.resolve({})),
}));

import { LicenseSection } from './LicenseSection';

function setTrial(trialStatus: MockTrialStatus | null) {
  state = { ...state, trialStatus };
}

beforeEach(() => {
  activateLicense.mockReset();
  startTrial.mockReset();
  state = {
    tier: 'free',
    trialStatus: null,
    licenseLoading: false,
    activateLicense,
    startTrial,
    loadLicense: vi.fn(() => Promise.resolve()),
    loadTrialStatus: vi.fn(() => Promise.resolve()),
    expired: false,
    daysRemaining: 0,
    expiresAt: null,
    licenseLoaded: true,
    licenseLoadError: null,
    recoverLicenseByEmail: vi.fn(),
  };
});

async function activate(onStatus: (s: string) => void, key = '4DA-bad') {
  render(<LicenseSection onStatus={onStatus} />);
  fireEvent.change(screen.getByPlaceholderText('XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXXXX-V3'), { target: { value: key } });
  await act(async () => {
    fireEvent.click(screen.getByText('action.activate'));
  });
}

describe('LicenseSection — activation errors', () => {
  it('reports a refused key through the errors namespace, with the backend reason', async () => {
    activateLicense.mockResolvedValue({ ok: false, reason: 'Too many activation attempts. Please wait a minute before trying again.' });
    const onStatus = vi.fn();
    await activate(onStatus);

    expect(onStatus).toHaveBeenCalledWith('errors:error.license.validationFailed');
    // The reason itself is shown next to the field, not "Unknown error".
    expect(screen.getByText('Too many activation attempts. Please wait a minute before trying again.')).toBeInTheDocument();
  });

  it('falls back to the localized invalid-key line when no reason came back', async () => {
    activateLicense.mockResolvedValue({ ok: false });
    const onStatus = vi.fn();
    await activate(onStatus);

    expect(onStatus).toHaveBeenCalledWith('settings.license.invalidKey');
  });
});

describe('LicenseSection — trial button', () => {
  it('is not offered while the trial status is unknown', () => {
    render(<LicenseSection onStatus={vi.fn()} />);
    expect(screen.queryByText('settings.license.startTrial')).not.toBeInTheDocument();
  });

  it('is not offered once a trial has run', () => {
    setTrial({ active: false, days_remaining: 0, started_at: '2026-09-01T00:00:00Z', has_license: false });
    render(<LicenseSection onStatus={vi.fn()} />);
    expect(screen.queryByText('settings.license.startTrial')).not.toBeInTheDocument();
    expect(screen.getByText('settings.license.trialExpired')).toBeInTheDocument();
  });

  it('is offered when the backend confirms none has started', () => {
    setTrial({ active: false, days_remaining: 0, started_at: null, has_license: false });
    render(<LicenseSection onStatus={vi.fn()} />);
    expect(screen.getByText('settings.license.startTrial')).toBeInTheDocument();
  });

  it('says the trial already ran when that is why the start was refused', async () => {
    setTrial({ active: false, days_remaining: 0, started_at: null, has_license: false });
    // The store re-reads the status on a refusal (license-slice startTrial).
    startTrial.mockImplementation(() => {
      setTrial({ active: false, days_remaining: 0, started_at: '2026-09-01T00:00:00Z', has_license: false });
      return Promise.resolve(false);
    });
    const onStatus = vi.fn();
    render(<LicenseSection onStatus={onStatus} />);

    await act(async () => {
      fireEvent.click(screen.getByText('settings.license.startTrial'));
    });

    expect(onStatus).toHaveBeenCalledWith('pro.trialEnded');
  });

  it('reports a start that failed for another reason as a failure', async () => {
    setTrial({ active: false, days_remaining: 0, started_at: null, has_license: false });
    startTrial.mockResolvedValue(false);
    const onStatus = vi.fn();
    render(<LicenseSection onStatus={onStatus} />);

    await act(async () => {
      fireEvent.click(screen.getByText('settings.license.startTrial'));
    });

    expect(onStatus).toHaveBeenCalledWith('settings.license.trialError');
  });
});
