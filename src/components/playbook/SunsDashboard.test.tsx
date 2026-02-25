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
    loadSunStatuses: vi.fn(),
    loadSunAlerts: vi.fn(),
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
  // Note: t() mock from setup.ts returns the i18n key string as-is

  it('renders the header', () => {
    setMockState({});
    render(<SunsDashboard />);
    // t('suns.title') renders as 'suns.title' in test mock
    expect(screen.getByText('suns.title')).toBeInTheDocument();
    // t('suns.active', {...}) renders as 'suns.active'
    expect(screen.getByText('suns.active')).toBeInTheDocument();
  });

  it('renders sun rows when statuses exist', () => {
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
    // Active badge should show count info
    expect(screen.getByText('suns.active')).toBeInTheDocument();
  });

  it('renders alert rows when alerts exist', () => {
    setMockState({
      sunAlerts: [
        { id: 1, sun_id: 'sun-1', alert_type: 'failure', message: 'Connection failed', acknowledged: false, created_at: new Date().toISOString() },
      ],
    });
    render(<SunsDashboard />);
    expect(screen.getByText(/Connection failed/)).toBeInTheDocument();
    // Dismiss button uses t('action.dismiss')
    expect(screen.getByText('action.dismiss')).toBeInTheDocument();
  });

  it('calls loadStatuses and loadAlerts on mount', () => {
    const loadStatuses = vi.fn();
    const loadAlerts = vi.fn();
    setMockState({
      loadSunStatuses: loadStatuses,
      loadSunAlerts: loadAlerts,
    });
    render(<SunsDashboard />);
    expect(loadStatuses).toHaveBeenCalledTimes(1);
    expect(loadAlerts).toHaveBeenCalledTimes(1);
  });
});
