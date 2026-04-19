// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SystemHealthDot } from './SystemHealthDot';

// Mock the cmd function
const mockCmd = vi.fn();
vi.mock('../lib/commands', () => ({
  cmd: (...args: unknown[]) => mockCmd(...args),
}));

describe('SystemHealthDot', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders nothing when health check returns no issues', async () => {
    mockCmd.mockResolvedValueOnce([]);
    const { container } = render(<SystemHealthDot />);
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_startup_health');
    });
    // Healthy = no dot shown
    expect(container.querySelector('button')).not.toBeInTheDocument();
  });

  it('renders nothing when health check fails', async () => {
    mockCmd.mockRejectedValueOnce(new Error('Check failed'));
    const { container } = render(<SystemHealthDot />);
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_startup_health');
    });
    expect(container.querySelector('button')).not.toBeInTheDocument();
  });

  it('renders a warning dot when issues are warnings only', async () => {
    mockCmd.mockResolvedValueOnce([
      { severity: 'warning', component: 'embedding', message: 'Degraded' },
    ]);
    render(<SystemHealthDot />);
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    const button = screen.getByRole('button');
    expect(button.title).toContain('warning');
  });

  it('renders an error dot when errors exist', async () => {
    mockCmd.mockResolvedValueOnce([
      { severity: 'error', component: 'database', message: 'DB locked' },
    ]);
    render(<SystemHealthDot />);
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    const button = screen.getByRole('button');
    expect(button.title).toContain('error');
  });

  it('shows issue count in title', async () => {
    mockCmd.mockResolvedValueOnce([
      { severity: 'warning', component: 'embedding', message: 'Issue 1' },
      { severity: 'warning', component: 'settings', message: 'Issue 2' },
    ]);
    render(<SystemHealthDot />);
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    const button = screen.getByRole('button');
    expect(button.title).toContain('2');
  });

  it('calls onClick when clicked', async () => {
    mockCmd.mockResolvedValueOnce([
      { severity: 'warning', component: 'sources', message: 'No sources' },
    ]);
    const onClick = vi.fn();
    render(<SystemHealthDot onClick={onClick} />);
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('has accessible label matching the title', async () => {
    mockCmd.mockResolvedValueOnce([
      { severity: 'error', component: 'database', message: 'DB error' },
    ]);
    render(<SystemHealthDot />);
    await waitFor(() => {
      const button = screen.getByRole('button');
      expect(button.getAttribute('aria-label')).toBe(button.title);
    });
  });
});
