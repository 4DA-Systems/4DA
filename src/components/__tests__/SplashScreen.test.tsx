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

// Mock game components
vi.mock('../../lib/fourda-components', () => ({
  registerFourdaComponent: vi.fn(() => Promise.resolve()),
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
    mockInvoke.mockImplementation(() => new Promise(() => {})); // hang
    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={999999} />);
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

  it('calls onComplete after backend ready and min time elapsed', async () => {
    const onComplete = vi.fn();
    mockInvoke.mockResolvedValue({});

    render(<SplashScreen onComplete={onComplete} minimumDisplayTime={0} />);

    // Wait for backend stages and minimum display time to complete, then onComplete
    await waitFor(() => {
      expect(onComplete).toHaveBeenCalled();
    }, { timeout: 3000 });
  });

  it('shows ready state after backend initialization completes', async () => {
    mockInvoke.mockResolvedValue({});

    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);

    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveAttribute('aria-busy', 'false');
    }, { timeout: 3000 });

    unmount();
  });

  it('invokes get_settings during initialization', async () => {
    mockInvoke.mockResolvedValue({});

    const { unmount } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_settings', {});
    }, { timeout: 3000 });

    unmount();
  });

  it('handles backend errors gracefully and shows error state', async () => {
    const onComplete = vi.fn();
    mockInvoke.mockRejectedValue(new Error('Backend unavailable'));

    render(<SplashScreen onComplete={onComplete} minimumDisplayTime={0} />);

    // Should show error label (stays in error state with retry/refresh)
    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveAttribute('aria-label', 'splash.error');
    }, { timeout: 3000 });

    // Should NOT auto-complete — user must retry or refresh
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
