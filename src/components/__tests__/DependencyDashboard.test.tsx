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
import DependencyDashboard from '../DependencyDashboard';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = await import('../../lib/commands') as any;

const mockOverview = {
  total_dependencies: 42,
  total_projects: 2,
  direct_dependencies: 30,
  dev_dependencies: 12,
  ecosystems: [
    { ecosystem: 'rust', count: 20 },
    { ecosystem: 'javascript', count: 22 },
  ],
  projects: [
    { name: 'project-a', path: '/path/to/a', dependency_count: 20, alert_count: 1 },
    { name: 'project-b', path: '/path/to/b', dependency_count: 22, alert_count: 0 },
  ],
  alerts: { total: 1, critical: 1, high: 0, medium: 0, low: 0 },
  cross_project_packages: 3,
  cross_project_top: [
    { package_name: 'serde', ecosystem: 'rust', project_count: 2 },
  ],
};

const mockProjectDeps = {
  dependencies: [
    { name: 'serde', version: '1.0.0', ecosystem: 'rust', is_dev: false, alerts: [] },
    { name: 'tokio', version: '1.28.0', ecosystem: 'rust', is_dev: false, alerts: [{ id: 1, severity: 'critical', title: 'CVE-2024-001' }] },
  ],
};

const mockAlerts = {
  alerts: [
    { id: 1, package_name: 'tokio', ecosystem: 'rust', severity: 'critical', title: 'CVE-2024-001', alert_type: 'vulnerability' },
  ],
};

function setupMocks() {
  vi.mocked(cmd).mockImplementation((command: string) => {
    switch (command) {
      case 'get_dependency_overview':
        return Promise.resolve(mockOverview);
      case 'get_project_deps':
        return Promise.resolve(mockProjectDeps);
      case 'get_dependency_alerts':
        return Promise.resolve(mockAlerts);
      default:
        return Promise.resolve(null);
    }
  });
}

describe('DependencyDashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setupMocks();
  });

  it('renders the dashboard header', async () => {
    render(<DependencyDashboard />);
    await waitFor(() => {
      expect(screen.getByText('deps.title')).toBeInTheDocument();
    });
  });

  it('renders summary stat cards', async () => {
    render(<DependencyDashboard />);
    await waitFor(() => {
      expect(screen.getByText('deps.total')).toBeInTheDocument();
    });
    expect(screen.getByText('deps.direct')).toBeInTheDocument();
    expect(screen.getByText('deps.dev')).toBeInTheDocument();
    // "deps.alerts" appears in the stat card, "vulns.activeAlerts" in the alerts heading
    expect(screen.getByText('deps.alerts')).toBeInTheDocument();
  });

  it('renders the dependency table with column headers', async () => {
    render(<DependencyDashboard />);
    await waitFor(() => {
      expect(screen.getByText('Name')).toBeInTheDocument();
    });
    expect(screen.getByText('Version')).toBeInTheDocument();
    expect(screen.getByText('Ecosystem')).toBeInTheDocument();
    expect(screen.getByText('Type')).toBeInTheDocument();
  });

  it('renders the project selector when multiple projects exist', async () => {
    render(<DependencyDashboard />);
    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeInTheDocument();
    });
  });

  it('renders active alerts section with severity badges', async () => {
    render(<DependencyDashboard />);
    await waitFor(() => {
      expect(screen.getByText('vulns.activeAlerts')).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByText('critical')).toBeInTheDocument();
    });
  });
});
