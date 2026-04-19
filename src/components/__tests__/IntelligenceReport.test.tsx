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

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import IntelligenceReportCard from '../IntelligenceReport';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = await import('../../lib/commands') as any;

const mockIntelligenceReport = {
  period: '2026-03',
  accuracy_current: 0.874,
  accuracy_previous: 0.842,
  accuracy_delta: 0.032,
  topics_tracked: 19,
  topics_added: 3,
  noise_rejected: 1847,
  noise_rejection_pct: 92.3,
  time_saved_hours: 14.2,
  security_alerts: 3,
  security_acted_on: 2,
  decisions_recorded: 12,
  feedback_signals: 89,
};

function setupMocks() {
  vi.mocked(cmd).mockImplementation((command: string) => {
    switch (command) {
      case 'get_intelligence_report':
        return Promise.resolve(mockIntelligenceReport);
      default:
        return Promise.resolve(null);
    }
  });
}

describe('IntelligenceReportCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setupMocks();
  });

  it('renders the report title', async () => {
    render(<IntelligenceReportCard />);
    await waitFor(() => {
      expect(screen.getByText('report.title')).toBeInTheDocument();
    });
  });

  it('renders all key metric labels', async () => {
    render(<IntelligenceReportCard />);
    await waitFor(() => {
      expect(screen.getByText('report.relevanceAccuracy')).toBeInTheDocument();
    });
    expect(screen.getByText('report.topicsTracked')).toBeInTheDocument();
    expect(screen.getByText('report.noiseRejected')).toBeInTheDocument();
    expect(screen.getByText('report.timeSaved')).toBeInTheDocument();
  });

  it('renders secondary metrics', async () => {
    render(<IntelligenceReportCard />);
    await waitFor(() => {
      expect(screen.getByText('report.securityAlerts')).toBeInTheDocument();
    });
    expect(screen.getByText('report.decisions')).toBeInTheDocument();
  });

  it('renders the accuracy progress bar', async () => {
    render(<IntelligenceReportCard />);
    await waitFor(() => {
      expect(screen.getByRole('progressbar')).toBeInTheDocument();
    });
  });

  it('renders trend indicators', async () => {
    render(<IntelligenceReportCard />);
    await waitFor(() => {
      expect(screen.getByText('report.relevanceAccuracy')).toBeInTheDocument();
    });
    // The component renders delta values with +/- signs for trend indicators
    const deltas = screen.getAllByText(/[+-]\d/);
    expect(deltas.length).toBeGreaterThan(0);
  });
});
