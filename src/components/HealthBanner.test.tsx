// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { HealthBanner } from './HealthBanner';

// Mock the cmd function
const mockCmd = vi.fn();
vi.mock('../lib/commands', () => ({
  cmd: (...args: unknown[]) => mockCmd(...args),
}));

describe('HealthBanner', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders nothing when health check returns no issues', async () => {
    mockCmd.mockResolvedValueOnce([]);
    const { container } = render(<HealthBanner />);
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_startup_health');
    });
    expect(container.innerHTML).toBe('');
  });

  it('renders nothing when health check fails', async () => {
    mockCmd.mockRejectedValueOnce(new Error('Failed'));
    const { container } = render(<HealthBanner />);
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_startup_health');
    });
    expect(container.innerHTML).toBe('');
  });

  it('renders single issue message directly', async () => {
    mockCmd.mockResolvedValueOnce([
      { component: 'embedding', severity: 'warning', message: 'Ollama not running' },
    ]);
    render(<HealthBanner />);
    await waitFor(() => {
      expect(screen.getByText('Ollama not running')).toBeInTheDocument();
    });
  });

  it('renders issue count for multiple issues', async () => {
    mockCmd.mockResolvedValueOnce([
      { component: 'embedding', severity: 'warning', message: 'Ollama not running' },
      { component: 'database', severity: 'error', message: 'DB locked' },
    ]);
    render(<HealthBanner />);
    await waitFor(() => {
      expect(screen.getByText('health.issueCount')).toBeInTheDocument();
    });
  });

  it('dismisses banner when dismiss button is clicked', async () => {
    mockCmd.mockResolvedValueOnce([
      { component: 'settings', severity: 'warning', message: 'Config issue' },
    ]);
    render(<HealthBanner />);
    await waitFor(() => {
      expect(screen.getByText('Config issue')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText('action.dismiss'));
    expect(screen.queryByText('Config issue')).not.toBeInTheDocument();
  });

  it('expands to show details when header is clicked with multiple issues', async () => {
    mockCmd.mockResolvedValueOnce([
      { component: 'embedding', severity: 'warning', message: 'Ollama not running' },
      { component: 'database', severity: 'error', message: 'DB locked' },
    ]);
    render(<HealthBanner />);
    await waitFor(() => {
      expect(screen.getByText('health.issueCount')).toBeInTheDocument();
    });

    // Click to expand
    fireEvent.click(screen.getByText('health.issueCount'));

    // Both individual issue messages should be visible
    expect(screen.getByText('Ollama not running')).toBeInTheDocument();
    expect(screen.getByText('DB locked')).toBeInTheDocument();
  });

  it('shows fix hints for known components', async () => {
    mockCmd.mockResolvedValueOnce([
      { component: 'embedding', severity: 'warning', message: 'Embedding issue' },
      { component: 'database', severity: 'error', message: 'DB issue' },
    ]);
    render(<HealthBanner />);
    await waitFor(() => {
      expect(screen.getByText('health.issueCount')).toBeInTheDocument();
    });

    // Expand
    fireEvent.click(screen.getByText('health.issueCount'));

    // Fix hints for embedding and database
    expect(screen.getByText(/ollama pull/i)).toBeInTheDocument();
    expect(screen.getByText(/restarting the app/i)).toBeInTheDocument();
  });
});
