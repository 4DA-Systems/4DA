// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

const mockDepAlerts = {
  alerts: [
    {
      id: 1,
      package_name: 'tokio',
      ecosystem: 'cargo',
      alert_type: 'cve',
      severity: 'critical',
      title: 'CVE-2024-1234: Remote code execution in tokio',
      description: 'Remote code execution vulnerability in tokio runtime',
      affected_versions: '< 1.36.0',
      source_url: 'https://github.com/advisories/GHSA-test-1',
      detected_at: '2026-03-10T00:00:00Z',
    },
    {
      id: 2,
      package_name: 'serde',
      ecosystem: 'cargo',
      alert_type: 'cve',
      severity: 'high',
      title: 'CVE-2024-5678: Denial of service in serde',
      description: 'Denial of service via crafted input',
      affected_versions: '< 1.0.200',
      source_url: null,
      detected_at: '2026-03-12T00:00:00Z',
    },
  ],
  total: 2,
};

function setupMocks() {
  vi.mocked(cmd).mockImplementation((command: string) => {
    switch (command) {
      case 'get_dependency_alerts':
        return Promise.resolve(mockDepAlerts);
      case 'resolve_dependency_alert':
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
      expect(screen.getByText('security.severityCritical')).toBeInTheDocument();
    });
    expect(screen.getByText('security.severityHigh')).toBeInTheDocument();
    expect(screen.getByText('security.severityMedium')).toBeInTheDocument();
    expect(screen.getByText('security.severityLow')).toBeInTheDocument();
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
      const resolveButtons = screen.getAllByText('security.resolve');
      expect(resolveButtons.length).toBeGreaterThan(0);
    });
  });

  it('shows resolved section after resolving an alert', async () => {
    render(<SecurityDashboard />);
    await waitFor(() => {
      expect(screen.getAllByText('security.resolve').length).toBeGreaterThan(0);
    });
    // Resolve first alert
    fireEvent.click(screen.getAllByText('security.resolve')[0]!);
    await waitFor(() => {
      expect(screen.getByText('security.resolved')).toBeInTheDocument();
    });
  });

  it('moves alert to resolved when Resolve is clicked', async () => {
    render(<SecurityDashboard />);
    await waitFor(() => {
      expect(screen.getAllByText('security.resolve').length).toBe(2);
    });
    const resolveButtons = screen.getAllByText('security.resolve');
    fireEvent.click(resolveButtons[0]!);
    await waitFor(() => {
      const remaining = screen.getAllByText('security.resolve');
      expect(remaining.length).toBe(1);
    });
  });
});
