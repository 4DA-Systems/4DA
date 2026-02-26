import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SunsDashboard } from './SunsDashboard';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Configurable mock state
let mockState: Record<string, unknown> = {};
function setMockState(overrides: Record<string, unknown>) {
  mockState = {
    sunStatuses: [],
    sunAlerts: [],
    sunsLoading: false,
    streetHealth: null,
    loadSunStatuses: vi.fn(),
    loadSunAlerts: vi.fn(),
    loadStreetHealth: vi.fn(),
    toggleSun: vi.fn(),
    acknowledgeSunAlert: vi.fn(),
    triggerSun: vi.fn(),
    ...overrides,
  };
}

vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

describe('SunsDashboard', () => {
  it('renders the header', () => {
    setMockState({});
    render(<SunsDashboard />);
    expect(screen.getByText('Suns')).toBeInTheDocument();
    expect(screen.getByText(/0\/0 active/)).toBeInTheDocument();
  });

  it('renders sun names when statuses exist', () => {
    setMockState({
      sunStatuses: [
        { id: 'sun-1', name: 'Sun A', module_id: 'S', enabled: true, interval_secs: 3600, last_run: null, next_run_in_secs: null, last_result: null, run_count: 0 },
        { id: 'sun-2', name: 'Sun B', module_id: 'R', enabled: false, interval_secs: 7200, last_run: null, next_run_in_secs: null, last_result: null, run_count: 0 },
      ],
    });
    render(<SunsDashboard />);
    expect(screen.getByText('Sun A')).toBeInTheDocument();
    expect(screen.getByText('Sun B')).toBeInTheDocument();
  });

  it('shows active count in badge', () => {
    setMockState({
      sunStatuses: [
        { id: 'sun-1', name: 'Sun A', module_id: 'S', enabled: true, interval_secs: 3600, last_run: null, next_run_in_secs: null, last_result: null, run_count: 0 },
        { id: 'sun-2', name: 'Sun B', module_id: 'R', enabled: false, interval_secs: 7200, last_run: null, next_run_in_secs: null, last_result: null, run_count: 0 },
      ],
    });
    render(<SunsDashboard />);
    expect(screen.getByText(/1\/2 active/)).toBeInTheDocument();
  });

  it('groups suns by module', () => {
    setMockState({
      sunStatuses: [
        { id: 'sun-1', name: 'Sun A', module_id: 'S', enabled: true, interval_secs: 3600, last_run: null, next_run_in_secs: null, last_result: null, run_count: 0 },
        { id: 'sun-2', name: 'Sun B', module_id: 'S', enabled: true, interval_secs: 7200, last_run: null, next_run_in_secs: null, last_result: null, run_count: 0 },
        { id: 'sun-3', name: 'Sun C', module_id: 'R', enabled: false, interval_secs: 3600, last_run: null, next_run_in_secs: null, last_result: null, run_count: 0 },
      ],
    });
    render(<SunsDashboard />);
    expect(screen.getByText('Sovereignty')).toBeInTheDocument();
    expect(screen.getByText('Revenue')).toBeInTheDocument();
    expect(screen.getByText('Sun A')).toBeInTheDocument();
    expect(screen.getByText('Sun C')).toBeInTheDocument();
  });

  it('renders alert rows when alerts exist', () => {
    setMockState({
      sunAlerts: [
        { id: 1, sun_id: 'sun-1', alert_type: 'failure', message: 'Connection failed', acknowledged: false, created_at: new Date().toISOString() },
      ],
    });
    render(<SunsDashboard />);
    expect(screen.getByText(/Connection failed/)).toBeInTheDocument();
    expect(screen.getByText('Dismiss')).toBeInTheDocument();
  });

  it('calls load functions on mount', () => {
    const loadStatuses = vi.fn();
    const loadAlerts = vi.fn();
    const loadHealth = vi.fn();
    setMockState({
      loadSunStatuses: loadStatuses,
      loadSunAlerts: loadAlerts,
      loadStreetHealth: loadHealth,
    });
    render(<SunsDashboard />);
    expect(loadStatuses).toHaveBeenCalledTimes(1);
    expect(loadAlerts).toHaveBeenCalledTimes(1);
    expect(loadHealth).toHaveBeenCalledTimes(1);
  });
});
