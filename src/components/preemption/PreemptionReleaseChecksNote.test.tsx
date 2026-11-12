// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { AdvisorySync } from '../../../src-tauri/bindings/bindings/AdvisorySync';
import { PreemptionReleaseChecksNote } from './PreemptionReleaseChecksNote';

const READY: AdvisorySync = {
  state: 'ready',
  ecosystems: 2,
  synced_at: '2026-11-12 01:00:00',
  error: null,
  in_flight: false,
  skipped_temp: 0,
};

describe('PreemptionReleaseChecksNote (audit P1)', () => {
  it('says nothing while every release lookup is made', () => {
    const { container } = render(<PreemptionReleaseChecksNote sync={READY} />);
    expect(container.firstChild).toBeNull();
    const { container: none } = render(<PreemptionReleaseChecksNote sync={undefined} />);
    expect(none.firstChild).toBeNull();
  });

  it('states that release checks are off when registry lookups are off', () => {
    render(<PreemptionReleaseChecksNote sync={{ ...READY, registry_lookups_off: true }} />);
    expect(screen.getByTestId('preemption-release-checks')).toBeInTheDocument();
    expect(screen.getByText('preemption.releaseChecks.offTitle')).toBeInTheDocument();
    expect(screen.getByText('preemption.releaseChecks.offBody')).toBeInTheDocument();
  });

  it('counts private packages that are never looked up', () => {
    render(<PreemptionReleaseChecksNote sync={{ ...READY, private_not_looked_up: 3 }} />);
    expect(screen.getByText('preemption.releaseChecks.privateTitle')).toBeInTheDocument();
    expect(screen.getByText('preemption.releaseChecks.privateBody')).toBeInTheDocument();
  });
});
