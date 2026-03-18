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

const mockAccuracyMetrics = {
  precision: 0.872,
  calibration_error: -0.03,
};

const mockAutophagyStatus = {
  total_cycles: 5,
  total_anti_patterns: 2,
  last_cycle: {
    items_analyzed: 100,
    items_pruned: 35,
  },
};

const mockCompoundAdvantage = {
  calibration_accuracy: 0.88,
  trend: 1,
  avg_lead_time_hours: 4.2,
  windows_opened: 12,
  windows_acted: 8,
  items_surfaced: 1500,
};

function setupMocks() {
  vi.mocked(cmd).mockImplementation((command: string) => {
    switch (command) {
      case 'ace_get_accuracy_metrics':
        return Promise.resolve(mockAccuracyMetrics);
      case 'get_autophagy_status':
        return Promise.resolve(mockAutophagyStatus);
      case 'get_compound_advantage':
        return Promise.resolve(mockCompoundAdvantage);
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
      expect(screen.getByText('Your Intelligence This Month')).toBeInTheDocument();
    });
  });

  it('renders all key metric labels', async () => {
    render(<IntelligenceReportCard />);
    await waitFor(() => {
      expect(screen.getByText('Relevance Accuracy')).toBeInTheDocument();
    });
    expect(screen.getByText('Calibration Accuracy')).toBeInTheDocument();
    expect(screen.getByText('Noise Rejected')).toBeInTheDocument();
    expect(screen.getByText('Lead Time')).toBeInTheDocument();
  });

  it('renders secondary metrics', async () => {
    render(<IntelligenceReportCard />);
    await waitFor(() => {
      expect(screen.getByText('Autophagy Cycles')).toBeInTheDocument();
    });
    expect(screen.getByText('Decision Windows')).toBeInTheDocument();
    expect(screen.getByText('Anti-patterns')).toBeInTheDocument();
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
      expect(screen.getByText('Relevance Accuracy')).toBeInTheDocument();
    });
    // The component renders delta values with +/- signs for trend indicators
    const deltas = screen.getAllByText(/[+-]\d/);
    expect(deltas.length).toBeGreaterThan(0);
  });
});
