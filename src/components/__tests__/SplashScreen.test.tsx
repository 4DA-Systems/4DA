// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

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

// Mock sun-logo image
vi.mock('../../assets/sun-logo.webp', () => ({
  default: 'mock-sun-logo.webp',
}));

// Mock error messages
vi.mock('../../utils/error-messages', () => ({
  translateError: (e: unknown) => String(e),
}));


// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { SplashScreen } from '../SplashScreen';
import { invoke } from '@tauri-apps/api/core';

const mockInvoke = vi.mocked(invoke);

describe('SplashScreen', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders without crash', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
    unmount();
  });

  it('displays 4DA brand name', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    expect(screen.getByText('4DA')).toBeInTheDocument();
    unmount();
  });

  it('displays the app tagline', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    expect(screen.getByText('app.tagline')).toBeInTheDocument();
    unmount();
  });

  it('displays the version text', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    expect(screen.getByText('splash.version')).toBeInTheDocument();
    unmount();
  });

  it('has a progress bar', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
    unmount();
  });

  it('starts with aria-busy true', async () => {
    const { unmount } = render(<SplashScreen backendReady={false} onComplete={vi.fn()} minimumDisplayTime={999999} />);
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true');
    unmount();
  });

  it('has an aria-label with stage text', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-label');
    unmount();
  });

  it('shows the brand logo', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    const brand = screen.getByText('4DA');
    expect(brand).toBeInTheDocument();
    unmount();
  });

  it('has a refresh button for stuck state', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    const refreshBtn = screen.getByLabelText('splash.refreshIfStuck');
    expect(refreshBtn).toBeInTheDocument();
    unmount();
  });

  it('calls onComplete after observed backend readiness and min time elapsed', async () => {
    const onComplete = vi.fn();

    render(<SplashScreen onComplete={onComplete} minimumDisplayTime={0} />);

    // Wait for backend stages and minimum display time to complete, then onComplete
    await waitFor(() => {
      expect(onComplete).toHaveBeenCalled();
    }, { timeout: 3000 });
  });

  it('shows ready state after observed backend readiness', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);

    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'false');
    }, { timeout: 3000 });

    unmount();
  });

  it('does not invoke command IPC during initialization', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);

    expect(mockInvoke).not.toHaveBeenCalled();

    unmount();
  });

  it('waits without error while backend readiness is still unresolved', async () => {
    const onComplete = vi.fn();

    render(<SplashScreen backendReady={false} onComplete={onComplete} minimumDisplayTime={0} />);

    expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'true');
    expect(screen.queryByText('action.retry')).not.toBeInTheDocument();
    expect(onComplete).not.toHaveBeenCalled();
  });

  it('shows stage indicator dots', async () => {
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    // Stage indicator dots exist within the status region
    const status = screen.getByRole('status');
    const container = status.querySelector('div[style*="gap: 0.5rem"]');
    expect(container).toBeInTheDocument();
    unmount();
  });
});
