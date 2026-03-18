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
import DependencyDashboard from '../DependencyDashboard';

describe('DependencyDashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the dashboard header', () => {
    render(<DependencyDashboard />);
    expect(screen.getByText('Dependency Health')).toBeInTheDocument();
  });

  it('renders summary stat cards', () => {
    render(<DependencyDashboard />);
    expect(screen.getByText('Total')).toBeInTheDocument();
    expect(screen.getByText('Fresh')).toBeInTheDocument();
    expect(screen.getByText('Stale')).toBeInTheDocument();
    expect(screen.getByText('Vulnerable')).toBeInTheDocument();
  });

  it('renders the dependency table with column headers', () => {
    render(<DependencyDashboard />);
    expect(screen.getByText('Name')).toBeInTheDocument();
    expect(screen.getByText('Version')).toBeInTheDocument();
    expect(screen.getByText('Ecosystem')).toBeInTheDocument();
    expect(screen.getByText('Freshness')).toBeInTheDocument();
  });

  it('renders the project selector', () => {
    render(<DependencyDashboard />);
    expect(screen.getByRole('combobox')).toBeInTheDocument();
  });

  it('renders active alerts section with severity badges', () => {
    render(<DependencyDashboard />);
    expect(screen.getByText('Active Alerts')).toBeInTheDocument();
    expect(screen.getByText('critical')).toBeInTheDocument();
  });
});
