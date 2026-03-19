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
import { ConvergenceTab } from '../intelligence/ConvergenceTab';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = await import('../../lib/commands') as any;

describe('ConvergenceTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading skeleton initially', () => {
    vi.mocked(cmd).mockReturnValue(new Promise(() => {}));
    const { container } = render(<ConvergenceTab />);
    expect(container.querySelector('.animate-pulse')).toBeInTheDocument();
  });

  it('shows empty state when no project data exists', async () => {
    // Reject all commands so allSettled yields {status:'rejected'} for each,
    // leaving state at initial values (null/null/[]).
    vi.mocked(cmd).mockRejectedValue(new Error('no data'));
    render(<ConvergenceTab />);
    await waitFor(() => {
      expect(screen.getByText('No project data available')).toBeInTheDocument();
    });
    expect(screen.getByText(/Run ACE discovery/)).toBeInTheDocument();
  });

  it('renders convergence stats and shared technologies', async () => {
    vi.mocked(cmd).mockImplementation((command: string) => {
      switch (command) {
        case 'get_tech_convergence':
          return Promise.resolve({
            total_projects: 3,
            shared_technologies: [
              { name: 'TypeScript', category: 'language', project_count: 3, adoption_pct: 1.0 },
              { name: 'React', category: 'framework', project_count: 2, adoption_pct: 0.67 },
            ],
            unique_technologies: [
              { name: 'Rust', category: 'language', project_path: '/proj-a', bus_factor_risk: 'medium' },
            ],
            convergence_score: 0.72,
          });
        case 'get_project_health_comparison':
          return Promise.resolve({ projects: [] });
        case 'get_cross_project_dependencies':
          return Promise.resolve([]);
        default:
          return Promise.resolve(null);
      }
    });

    render(<ConvergenceTab />);

    await waitFor(() => {
      expect(screen.getByText('Projects')).toBeInTheDocument();
    });
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText('Shared Tech')).toBeInTheDocument();
    expect(screen.getByText('Convergence')).toBeInTheDocument();
    expect(screen.getByText('Shared Technologies')).toBeInTheDocument();
    expect(screen.getByText('TypeScript')).toBeInTheDocument();
    expect(screen.getByText('React')).toBeInTheDocument();
    expect(screen.getByText('Unique to Single Projects')).toBeInTheDocument();
    expect(screen.getByText('Rust')).toBeInTheDocument();
  });

  it('renders cross-project dependencies table', async () => {
    vi.mocked(cmd).mockImplementation((command: string) => {
      if (command === 'get_cross_project_dependencies') {
        return Promise.resolve([
          { name: 'serde', ecosystem: 'rust', projects: ['/a', '/b'], project_count: 2 },
        ]);
      }
      return Promise.resolve(null);
    });

    render(<ConvergenceTab />);

    await waitFor(() => {
      expect(screen.getByText('Cross-Project Dependencies')).toBeInTheDocument();
    });
    expect(screen.getByText('serde')).toBeInTheDocument();
    expect(screen.getByText('rust')).toBeInTheDocument();
  });
});
