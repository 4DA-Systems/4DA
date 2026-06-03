// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// The first-run personalize nudge is a skipper's recovery path: a user who declined
// the onboarding project scan must be able to run it in one click (fully local), not
// only by hunting through Settings. (t() is mocked to return the key in tests.)

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { PersonalizeNudge } from './PersonalizeNudge';

describe('PersonalizeNudge', () => {
  const baseProps = {
    onScanProjects: vi.fn(),
    onOpenSettings: vi.fn(),
    onDismiss: vi.fn(),
    isScanning: false,
  };

  it('offers a one-click project scan as the primary action', () => {
    render(<PersonalizeNudge {...baseProps} />);
    expect(screen.getByText('onboarding.choice.scanProjects')).toBeInTheDocument();
  });

  it('runs the scan on click', () => {
    const onScanProjects = vi.fn();
    render(<PersonalizeNudge {...baseProps} onScanProjects={onScanProjects} />);
    fireEvent.click(screen.getByText('onboarding.choice.scanProjects'));
    expect(onScanProjects).toHaveBeenCalledTimes(1);
  });

  it('keeps Settings as a secondary, manual path', () => {
    const onOpenSettings = vi.fn();
    render(<PersonalizeNudge {...baseProps} onOpenSettings={onOpenSettings} />);
    fireEvent.click(screen.getByText('header.settings'));
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
  });

  it('is dismissible (consent stays the user\'s — INV-004)', () => {
    const onDismiss = vi.fn();
    render(<PersonalizeNudge {...baseProps} onDismiss={onDismiss} />);
    fireEvent.click(screen.getByLabelText('action.dismiss'));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it('shows a local-only scanning state and hides the scan button while scanning', () => {
    render(<PersonalizeNudge {...baseProps} isScanning={true} />);
    expect(screen.getByRole('status')).toHaveTextContent('onboarding.choice.scanning');
    expect(screen.queryByText('onboarding.choice.scanProjects')).not.toBeInTheDocument();
    // Dismiss is disabled mid-scan so the card can't be torn out from under the run.
    expect(screen.getByLabelText('action.dismiss')).toBeDisabled();
  });
});
