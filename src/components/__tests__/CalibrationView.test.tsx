import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn(),
}));

vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      setShowSettings: vi.fn(),
      setActiveView: vi.fn(),
    }),
  ),
}));

vi.mock('../calibration/CalibrationComponents', () => ({
  DimensionBar: ({ label, score }: { label: string; score: number }) => (
    <div data-testid="dimension-bar">{label}: {score}</div>
  ),
  StatusRow: ({ label, ok, detail }: { label: string; ok: boolean; detail: string }) => (
    <div data-testid="status-row">{label}: {ok ? 'OK' : 'NO'} - {detail}</div>
  ),
  RecommendationItem: ({ rec }: { rec: { label: string } }) => (
    <div data-testid="recommendation">{rec.label}</div>
  ),
}));

vi.mock('../calibration/calibration-utils', () => ({
  gradeColor: (grade: string) => grade === 'A' ? '#22C55E' : '#FFFFFF',
}));

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(),
}));

import { CalibrationView } from '../CalibrationView';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = await import('../../lib/commands') as any;

const mockResult = {
  grade: 'B',
  grade_score: 72,
  infrastructure_score: 80,
  context_richness_score: 60,
  signal_coverage_score: 75,
  discrimination_score: 70,
  active_signal_axes: ['hackernews', 'github'],
  rig_requirements: {
    ollama_running: true,
    embedding_available: true,
    embedding_model: 'nomic-embed-text',
    can_reach_grade_a: false,
    estimated_ram_gb: 8,
    recommended_model: 'nomic-embed-text',
    grade_a_requirements: ['Add more interests'],
  },
  recommendations: [
    { label: 'Add interests', impact: 'high', action_type: 'open_settings_interests' },
  ],
};

describe('CalibrationView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders title and subtitle', async () => {
    vi.mocked(cmd).mockResolvedValue(mockResult);
    render(<CalibrationView />);
    expect(screen.getByText('calibration.title')).toBeInTheDocument();
    expect(screen.getByText('calibration.subtitle')).toBeInTheDocument();
  });

  it('shows loading state initially', () => {
    vi.mocked(cmd).mockReturnValue(new Promise(() => {})); // never resolves
    render(<CalibrationView />);
    const loadingEl = screen.getByText('calibration.analyzing');
    expect(loadingEl).toBeInTheDocument();
    expect(loadingEl.closest('[role="status"]')).toBeTruthy();
  });

  it('shows error with role="alert" on failure', async () => {
    vi.mocked(cmd).mockRejectedValue(new Error('Network error'));
    render(<CalibrationView />);
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });
  });

  it('renders grade card with role="status" on success', async () => {
    vi.mocked(cmd).mockResolvedValue(mockResult);
    render(<CalibrationView />);
    await waitFor(() => {
      expect(screen.getByText('B')).toBeInTheDocument();
    });
    // Grade card has role="status"
    const gradeStatus = screen.getByRole('status', { name: /calibration\.ariaGradeScore/ });
    expect(gradeStatus).toBeInTheDocument();
  });

  it('re-calibrate button has proper aria-label', async () => {
    vi.mocked(cmd).mockResolvedValue(mockResult);
    render(<CalibrationView />);
    await waitFor(() => {
      expect(screen.getByText('B')).toBeInTheDocument();
    });
    const btn = screen.getByLabelText('calibration.reCalibrate');
    expect(btn).toBeInTheDocument();
  });

  it('renders dimension bars with i18n labels', async () => {
    vi.mocked(cmd).mockResolvedValue(mockResult);
    render(<CalibrationView />);
    await waitFor(() => {
      const bars = screen.getAllByTestId('dimension-bar');
      expect(bars).toHaveLength(4);
    });
  });

  it('renders rig requirements status rows', async () => {
    vi.mocked(cmd).mockResolvedValue(mockResult);
    render(<CalibrationView />);
    await waitFor(() => {
      const rows = screen.getAllByTestId('status-row');
      expect(rows.length).toBeGreaterThanOrEqual(4);
    });
  });

  it('renders recommendations', async () => {
    vi.mocked(cmd).mockResolvedValue(mockResult);
    render(<CalibrationView />);
    await waitFor(() => {
      expect(screen.getByTestId('recommendation')).toBeInTheDocument();
    });
  });

  it('renders fully calibrated message when no recommendations', async () => {
    vi.mocked(cmd).mockResolvedValue({ ...mockResult, grade: 'A+', recommendations: [] });
    render(<CalibrationView />);
    await waitFor(() => {
      const calibrated = screen.getByRole('status', { name: /calibration\.fullyCalibrated/ });
      expect(calibrated).toBeInTheDocument();
    });
  });
});
