// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { UpdateBanner } from './UpdateBanner';

describe('UpdateBanner', () => {
  const defaultProps = {
    update: { version: '2.1.0', body: 'Bug fixes and improvements', canAutoUpdate: true },
    installing: false,
    onInstall: vi.fn(),
    onDismiss: vi.fn(),
  };

  it('renders the version number', () => {
    render(<UpdateBanner {...defaultProps} />);
    expect(screen.getByText('update.available')).toBeInTheDocument();
  });

  it('renders the release body text', () => {
    render(<UpdateBanner {...defaultProps} />);
    expect(screen.getByText('Bug fixes and improvements')).toBeInTheDocument();
  });

  it('shows default body when body is null', () => {
    render(<UpdateBanner {...defaultProps} update={{ version: '2.1.0', body: null }} />);
    expect(screen.getByText('update.defaultBody')).toBeInTheDocument();
  });

  it('calls onInstall when install button is clicked', () => {
    const onInstall = vi.fn();
    render(<UpdateBanner {...defaultProps} onInstall={onInstall} />);
    const installBtn = screen.getByLabelText('update.install');
    fireEvent.click(installBtn);
    expect(onInstall).toHaveBeenCalledTimes(1);
  });

  it('calls onDismiss when dismiss button is clicked', () => {
    const onDismiss = vi.fn();
    render(<UpdateBanner {...defaultProps} onDismiss={onDismiss} />);
    const dismissBtn = screen.getByLabelText('Dismiss update notification');
    fireEvent.click(dismissBtn);
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it('disables install button when installing is true', () => {
    render(<UpdateBanner {...defaultProps} installing={true} />);
    const installBtn = screen.getByLabelText('update.installing');
    expect(installBtn).toBeDisabled();
  });

  it('shows "Later" button that calls onDismiss', () => {
    const onDismiss = vi.fn();
    render(<UpdateBanner {...defaultProps} onDismiss={onDismiss} />);
    const laterBtn = screen.getByLabelText('update.later');
    fireEvent.click(laterBtn);
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it('renders download link when canAutoUpdate is false', () => {
    render(
      <UpdateBanner
        {...defaultProps}
        update={{ version: '2.1.0', body: 'New release', canAutoUpdate: false }}
      />,
    );
    const link = screen.getByText('update.download');
    expect(link).toBeInTheDocument();
    expect(link.closest('a')).toHaveAttribute('href', expect.stringContaining('github.com'));
    expect(link.closest('a')).toHaveAttribute('target', '_blank');
  });
});
