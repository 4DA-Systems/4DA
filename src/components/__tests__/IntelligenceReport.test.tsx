import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';

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

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import IntelligenceReportCard from '../IntelligenceReport';

describe('IntelligenceReportCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the report title', () => {
    render(<IntelligenceReportCard />);
    expect(screen.getByText('Your Intelligence This Month')).toBeInTheDocument();
  });

  it('renders all key metric labels', () => {
    render(<IntelligenceReportCard />);
    expect(screen.getByText('Relevance Accuracy')).toBeInTheDocument();
    expect(screen.getByText('Topics Tracked')).toBeInTheDocument();
    expect(screen.getByText('Noise Rejected')).toBeInTheDocument();
    expect(screen.getByText('Time Saved')).toBeInTheDocument();
  });

  it('renders security and decisions metrics', () => {
    render(<IntelligenceReportCard />);
    expect(screen.getByText('Security Alerts')).toBeInTheDocument();
    expect(screen.getByText('Decisions Recorded')).toBeInTheDocument();
    expect(screen.getByText('Feedback Signals')).toBeInTheDocument();
  });

  it('renders the accuracy progress bar', () => {
    render(<IntelligenceReportCard />);
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
  });

  it('renders trend indicators', () => {
    render(<IntelligenceReportCard />);
    const arrows = screen.getAllByText(/[^\s]*[+]/);
    expect(arrows.length).toBeGreaterThan(0);
  });
});
