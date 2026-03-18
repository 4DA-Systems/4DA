import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

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

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import SecurityDashboard from '../SecurityDashboard';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = await import('../../lib/commands') as any;

const mockDecisionWindows = [
  {
    id: 1,
    window_type: 'security_patch',
    title: 'CVE-2024-1234 in tokio',
    description: 'Remote code execution vulnerability in tokio runtime',
    urgency: 0.95,
    dependency: 'tokio',
    status: 'open',
    opened_at: '2026-03-10T00:00:00Z',
  },
  {
    id: 2,
    window_type: 'security_patch',
    title: 'CVE-2024-5678 in serde',
    description: 'Denial of service via crafted input',
    urgency: 0.75,
    dependency: 'serde',
    status: 'open',
    opened_at: '2026-03-12T00:00:00Z',
  },
  {
    id: 3,
    window_type: 'security_patch',
    title: 'CVE-2024-9999 in hyper',
    description: 'HTTP request smuggling vulnerability',
    urgency: 0.5,
    dependency: 'hyper',
    status: 'acted',
    opened_at: '2026-03-05T00:00:00Z',
  },
];

function setupMocks() {
  vi.mocked(cmd).mockImplementation((command: string) => {
    switch (command) {
      case 'get_decision_windows':
        return Promise.resolve(mockDecisionWindows);
      case 'act_on_decision_window':
        return Promise.resolve(null);
      default:
        return Promise.resolve(null);
    }
  });
}

describe('SecurityDashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setupMocks();
  });

  it('renders severity count badges', async () => {
    render(<SecurityDashboard />);
    await waitFor(() => {
      expect(screen.getByText('Critical')).toBeInTheDocument();
    });
    expect(screen.getByText('High')).toBeInTheDocument();
    expect(screen.getByText('Medium')).toBeInTheDocument();
    expect(screen.getByText('Low')).toBeInTheDocument();
  });

  it('renders active alerts with CVE identifiers', async () => {
    render(<SecurityDashboard />);
    await waitFor(() => {
      const cveElements = screen.getAllByText(/CVE-/);
      expect(cveElements.length).toBeGreaterThan(0);
    });
  });

  it('renders Resolve buttons for active alerts', async () => {
    render(<SecurityDashboard />);
    await waitFor(() => {
      const resolveButtons = screen.getAllByText('Resolve');
      expect(resolveButtons.length).toBeGreaterThan(0);
    });
  });

  it('renders resolved timeline section', async () => {
    render(<SecurityDashboard />);
    await waitFor(() => {
      expect(screen.getByText('Resolved')).toBeInTheDocument();
    });
  });

  it('moves alert to resolved when Resolve is clicked', async () => {
    render(<SecurityDashboard />);
    await waitFor(() => {
      expect(screen.getAllByText('Resolve').length).toBe(2);
    });
    const resolveButtons = screen.getAllByText('Resolve');
    fireEvent.click(resolveButtons[0]);
    await waitFor(() => {
      const remaining = screen.getAllByText('Resolve');
      expect(remaining.length).toBe(1);
    });
  });
});
